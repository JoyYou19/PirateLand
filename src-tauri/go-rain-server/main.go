package main

import (
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/anacrolix/torrent"
	"github.com/nwaples/rardecode"
)

var (
	client             *torrent.Client
	completedDownloads sync.Map
)

type ProgressUpdate struct {
	FilePath string
	Progress DownloadProgress
}

type TorrentControl struct {
	FilePath string
	Action   string
}

func progressManager() {
	progress := make(map[string]DownloadProgress)

	for {
		select {
		case update := <-progressUpdates:
			progress[update.FilePath] = update.Progress
		case control := <-torrentControl:
			if control.Action == "remove" {
				delete(progress, control.FilePath)
			}
		}
	}
}

var (
	progressUpdates = make(chan ProgressUpdate, 100)
	torrentControl  = make(chan TorrentControl, 10)
)

var downloadProgress = make(map[string]DownloadProgress)
var progressMutex = &sync.RWMutex{}

var swarmMutex sync.Mutex // Add a global mutex for KnownSwarm access

func uploadTorrentHandler(w http.ResponseWriter, r *http.Request) {
	// Ensure the request Content-Type is JSON
	if r.Header.Get("Content-Type") != "application/json" {
		http.Error(w, "Invalid Content-Type, expected application/json", http.StatusBadRequest)
		log.Println("Error: Invalid Content-Type")
		return
	}

	// Parse the JSON body
	var req struct {
		FilePath  string `json:"file_path"`
		GameTitle string `json:"game_title"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, "Failed to parse JSON body", http.StatusBadRequest)
		log.Println("Error parsing JSON:", err)
		return
	}

	// Add the torrent file to the client
	tor, err := client.AddTorrentFromFile(req.FilePath)
	if err != nil {
		http.Error(w, "Failed to add torrent", http.StatusInternalServerError)
		log.Println("Error adding torrent:", err)
		return
	}

	// Start downloading the torrent
	go func(filePath string, tor *torrent.Torrent, gameTitle string) {
		// Wait for the torrent to be fully loaded
		<-tor.GotInfo()
		tor.DownloadAll()

		previousBytes := tor.BytesCompleted()
		previousTime := time.Now()

		// Monitor progress
		for range time.Tick(time.Second) {
			info := tor.Info()
			if info == nil {
				continue
			}

			// Safely get the number of connected peers
			swarmMutex.Lock()
			peers := tor.KnownSwarm()
			peersConnected := len(peers)
			swarmMutex.Unlock()

			// Calculate download progress
			bytesCompleted := tor.BytesCompleted()
			totalLength := info.TotalLength()
			progress := float64(bytesCompleted) / float64(totalLength)

			// Calculate speed in MB/s
			currentTime := time.Now()
			timeDiff := currentTime.Sub(previousTime).Seconds()
			bytesDiff := bytesCompleted - previousBytes

			downloadSpeed := float64(bytesDiff) / timeDiff     // bytes per second
			downloadSpeedMBps := downloadSpeed / (1024 * 1024) // MB per second

			// Update previous values
			previousBytes = bytesCompleted
			previousTime = currentTime

			// Calculate ETA
			remainingBytes := totalLength - bytesCompleted
			var etaSeconds float64
			if downloadSpeed > 0 {
				etaSeconds = float64(remainingBytes) / downloadSpeed
			} else {
				etaSeconds = -1 // Indicate unknown ETA
			}

			// Convert ETA to a human-readable format
			eta := "Unknown"
			if etaSeconds >= 0 {
				etaDuration := time.Duration(etaSeconds) * time.Second
				eta = fmt.Sprintf("%02d:%02d:%02d",
					int(etaDuration.Hours()),
					int(etaDuration.Minutes())%60,
					int(etaDuration.Seconds())%60)
			}

			// Update progress
			progressMutex.Lock()
			downloadProgress[filePath] = DownloadProgress{
				Name:           gameTitle,
				Progress:       progress,
				SpeedMBps:      downloadSpeedMBps,
				PeersConnected: peersConnected,
				Downloaded:     bytesCompleted,
				TotalSize:      totalLength,
				ETA:            eta,
			}
			progressMutex.Unlock()

			// Log progress
			log.Printf("Torrent: %s, Status: Downloading, Progress: %.2f%%, Peers: %d, Speed: %.2f MB/s",
				gameTitle, progress*100, peersConnected, downloadSpeedMBps)

			if progress >= 1.0 {
				_, alreadyProcessed := completedDownloads.LoadOrStore(filePath, true) // Check if already processed
				if alreadyProcessed {
					log.Printf("Torrent already processed: %s", filePath)
					break
				}

				log.Printf("Processing completed torrent: %s", filePath)
				err := processGameFolder(getDefaultDownloadsDir(), gameTitle, filePath)
				if err != nil {
					log.Printf("Error processing game folder: %v", err)
				}

				// Remove from progress tracking
				progressMutex.Lock()
				delete(downloadProgress, filePath)
				progressMutex.Unlock()
				break
			}
		}
	}(req.FilePath, tor, req.GameTitle)

	w.WriteHeader(http.StatusOK)
	w.Write([]byte("Torrent added successfully"))
}

func getDefaultDownloadsDir() string {
	// Determine the default downloads directory based on the OS
	if homeDir, err := os.UserHomeDir(); err == nil {
		// Cross-platform path: ~/Downloads/PirateLand
		return filepath.Join(homeDir, "Downloads", "PirateLand")
	}
	// Fallback: current working directory
	return "./downloads"
}

type DownloadProgress struct {
	Name            string  `json:"name"`
	Progress        float64 `json:"progress"`        // Value between 0.0 and 1.0
	SpeedMBps       float64 `json:"speed_mb_ps"`     // Download speed in MB/s
	PeersConnected  int     `json:"peers_connected"` // Number of connected peers
	Downloaded      int64   `json:"downloaded"`
	TotalSize       int64   `json:"total_size"`
	ExtractProgress float64 `json:"extract_progress"`
	ETA             string  `json:"eta"`
}

func downloadProgressHandler(w http.ResponseWriter, r *http.Request) {
	// Collect all active downloads
	var progressList []DownloadProgress

	// Lock for reading from the downloadProgress map
	progressMutex.RLock()
	for _, progress := range downloadProgress {
		progressList = append(progressList, progress)
	}
	progressMutex.RUnlock()

	// Sort the progress list by name
	sort.Slice(progressList, func(i, j int) bool {
		return progressList[i].Name < progressList[j].Name
	})

	// Return progress as JSON
	w.Header().Set("Content-Type", "application/json")
	if err := json.NewEncoder(w).Encode(progressList); err != nil {
		http.Error(w, "Failed to encode progress list", http.StatusInternalServerError)
		log.Println("Error encoding JSON:", err)
	}
}

func dropTorrentHandler(w http.ResponseWriter, r *http.Request) {
	// Ensure the request Content-Type is JSON
	if r.Header.Get("Content-Type") != "application/json" {
		http.Error(w, "Invalid Content-Type, expected application/json", http.StatusBadRequest)
		log.Println("Error: Invalid Content-Type")
		return
	}

	// Parse the JSON body
	var req struct {
		FilePath string `json:"file_path"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, "Failed to parse JSON body", http.StatusBadRequest)
		log.Println("Error parsing JSON:", err)
		return
	}

	// Find the torrent by matching the file path
	var targetTorrent *torrent.Torrent
	for _, tor := range client.Torrents() {
		if tor.Name() == filepath.Base(req.FilePath) {
			targetTorrent = tor
			break
		}
	}

	// If the torrent is not found, return an error
	if targetTorrent == nil {
		http.Error(w, "Torrent not found", http.StatusNotFound)
		log.Printf("Torrent not found for file path: %s", req.FilePath)
		return
	}

	// Drop the torrent
	targetTorrent.Drop()
	log.Printf("Dropped torrent: %s", req.FilePath)

	// Remove the associated progress entry by name
	var keyToDelete string
	progressMutex.Lock()
	for key, progress := range downloadProgress {
		if progress.Name == req.FilePath {
			keyToDelete = key
			break
		}
	}
	if keyToDelete != "" {
		delete(downloadProgress, keyToDelete)
		log.Printf("Removed download progress for: %s", req.FilePath)
	} else {
		log.Printf("No matching download progress found for: %s", req.FilePath)
	}
	progressMutex.Unlock()

	w.WriteHeader(http.StatusOK)
	w.Write([]byte("Torrent dropped successfully"))
}

func main() {
	go progressManager()
	// Determine the default downloads directory
	dataDir := getDefaultDownloadsDir()

	// Ensure the directories exist
	if err := os.MkdirAll(dataDir, 0755); err != nil {
		log.Fatalf("Failed to create downloads directory: %v", err)
	}

	// Configure Torrent client
	config := torrent.NewDefaultClientConfig()
	config.DataDir = dataDir

	var err error
	client, err = torrent.NewClient(config)
	if err != nil {
		log.Fatalf("Failed to create torrent client: %v", err)
	}
	defer client.Close()

	// Define HTTP endpoints
	http.HandleFunc("/upload-torrent", func(w http.ResponseWriter, r *http.Request) {
		uploadTorrentHandler(w, r)
	})
	http.HandleFunc("/drop-torrent", func(w http.ResponseWriter, r *http.Request) {
		dropTorrentHandler(w, r)
	})

	http.HandleFunc("/downloads-progress", downloadProgressHandler)

	// Start the server
	log.Println("Torrent server is running on http://localhost:8091")
	log.Fatal(http.ListenAndServe(":8091", nil))
}

func processGameFolder(baseDir, gameTitle string, filePath string) error {
	// Construct the expected game folder path directly
	gameFolderPath := filepath.Join(baseDir, gameTitle)

	// Check if the game folder exists
	if _, err := os.Stat(gameFolderPath); os.IsNotExist(err) {
		return fmt.Errorf("game folder not found: %s", gameFolderPath)
	}

	log.Printf("Found game folder: %s", gameFolderPath)

	// Process .zip and .rar files in the game folder
	fileEntries, err := os.ReadDir(gameFolderPath)
	if err != nil {
		return fmt.Errorf("error reading game folder %s: %w", gameFolderPath, err)
	}

	for _, file := range fileEntries {
		if strings.HasSuffix(file.Name(), ".zip") || strings.HasSuffix(file.Name(), ".rar") {
			archivePath := filepath.Join(gameFolderPath, file.Name())
			extractDir := filepath.Join(gameFolderPath, "Extracted") // Extract into a subdirectory

			log.Printf("Attempting to extract archive: %s to %s", archivePath, extractDir)

			// Ensure the extraction directory exists
			if err := os.MkdirAll(extractDir, 0755); err != nil {
				return fmt.Errorf("failed to create extraction directory: %w", err)
			}

			// Extract the archive with debugging enabled
			err := extractArchive(archivePath, extractDir, "online-fix.me", filePath)
			if err != nil {
				log.Printf("Error during extraction: %v", err)
				return fmt.Errorf("error extracting archive %s: %w", archivePath, err)
			}

			log.Printf("Extraction complete: %s", extractDir)
		}
	}

	return nil
}

func extractArchive(archivePath, extractDir, password, filePath string) error {
	if strings.HasSuffix(archivePath, ".rar") {
		// Pass a callback to update extract progress
		return extractRar(archivePath, extractDir, password, func(currentProgress float64) {
			// Safely update the progress using the mutex
			progressMutex.Lock()
			defer progressMutex.Unlock()

			// Check if the file exists in the downloadProgress map
			dp, ok := downloadProgress[filePath]
			if ok {
				// Update only the ExtractProgress field
				dp.ExtractProgress = currentProgress

				// Store the updated value back in the map
				downloadProgress[filePath] = dp
			}
		})
	} else {
		return fmt.Errorf("unsupported archive format: %s", archivePath)
	}
}

// Extract .rar files using rardecode
func extractRar(archivePath, extractDir, password string, updateProgress func(progress float64)) error {
	// Open the .rar file
	file, err := os.Open(archivePath)
	if err != nil {
		return fmt.Errorf("failed to open .rar file: %w", err)
	}
	defer file.Close()

	// Create a new RAR reader
	reader, err := rardecode.NewReader(file, password)
	if err != nil {
		return fmt.Errorf("failed to create RAR reader: %w", err)
	}

	// Calculate the total size of the archive contents
	var totalSize int64
	file.Seek(0, io.SeekStart) // Reset to the beginning of the archive
	sizeReader, err := rardecode.NewReader(file, password)
	if err != nil {
		return fmt.Errorf("failed to create size reader: %w", err)
	}

	for {
		header, err := sizeReader.Next()
		if err != nil {
			if err == io.EOF {
				break // End of archive
			}
			return fmt.Errorf("error reading RAR archive size: %w", err)
		}
		if !header.IsDir {
			totalSize += header.UnPackedSize
		}
	}

	// Reset the file to the beginning of the archive
	file.Seek(0, io.SeekStart)
	reader, err = rardecode.NewReader(file, password)
	if err != nil {
		return fmt.Errorf("failed to reset RAR reader: %w", err)
	}

	// Extract files while tracking progress
	var extractedSize int64

	for {
		header, err := reader.Next()
		if err != nil {
			if err == io.EOF {
				break // End of archive
			}
			return fmt.Errorf("error reading RAR archive: %w", err)
		}

		// Determine the output path
		outputPath := filepath.Join(extractDir, header.Name)
		if header.IsDir {
			// Create directories if necessary
			if err := os.MkdirAll(outputPath, 0755); err != nil {
				return fmt.Errorf("failed to create directory: %w", err)
			}
		} else {
			// Create the file
			if err := os.MkdirAll(filepath.Dir(outputPath), 0755); err != nil {
				return fmt.Errorf("failed to create directory for file: %w", err)
			}

			outFile, err := os.Create(outputPath)
			if err != nil {
				return fmt.Errorf("failed to create file: %w", err)
			}

			// Copy the file contents
			bytesCopied, err := io.Copy(outFile, reader)
			if err != nil {
				outFile.Close()
				return fmt.Errorf("error writing file: %w", err)
			}
			outFile.Close()

			// Update progress
			extractedSize += bytesCopied
			progress := float64(extractedSize) / float64(totalSize)
			updateProgress(progress)
		}
	}

	// Ensure progress is set to 100% only after successful extraction
	updateProgress(1.0)
	log.Printf("Successfully extracted .rar file to: %s", extractDir)
	return nil
}

func simulateProgress(extractDir string, totalSize int64) {
	ticker := time.NewTicker(time.Second)
	defer ticker.Stop()

	for range ticker.C {
		// Calculate extracted size so far
		size, err := calculateDirSize(extractDir)
		if err != nil {
			log.Printf("Error calculating extracted size: %v", err)
			continue
		}

		// Simulate progress percentage
		progress := float64(size) / float64(totalSize) * 100
		log.Printf("Extraction progress: %.2f%%", progress)

		// Stop when progress reaches or exceeds 100%
		if progress >= 100 {
			break
		}
	}
}

func calculateDirSize(dir string) (int64, error) {
	var size int64
	err := filepath.Walk(dir, func(_ string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		if !info.IsDir() {
			size += info.Size()
		}
		return nil
	})
	return size, err
}
