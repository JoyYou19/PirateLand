import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// Define a type for the download progress data
interface Download {
  name: string;
  progress: number; // Progress is a number between 0 and 1
  speed_mb_ps: number; // Download speed in MB/s
  peers_connected: number; // Number of connected peers
  downloaded: number;
  total_size: number;
  extract_progress: number;
  eta: string;
}

interface GameDetailsData {
  header_image?: string;
}

function Downloads() {
  const [downloads, setDownloads] = useState<Download[]>([]);
  const [gameDetails, setGameDetails] = useState<
    Record<string, GameDetailsData>
  >({});
  const processedNames = useRef<Set<string>>(new Set()); // Track already processed names

  // Fetch download progress from the Go server
  useEffect(() => {
    const fetchDownloads = async () => {
      try {
        const response: Download[] = await invoke("get_downloads"); // Type the response
        setDownloads(response);
      } catch (error) {
        console.error("Failed to fetch downloads:", error);
      }
    };

    // Fetch downloads every second
    const interval = setInterval(fetchDownloads, 1000);

    return () => clearInterval(interval); // Cleanup on unmount
  }, []);

  // Fetch game details for new downloads
  useEffect(() => {
    const fetchDetailsForNewDownloads = async () => {
      for (const download of downloads) {
        const gameName = download.name;

        // Skip if the name has already been processed
        if (processedNames.current.has(gameName)) continue;

        try {
          const details: GameDetailsData | null = await invoke(
            "find_and_get_game_details_library",
            {
              query: gameName, // Pass the name to the query
            },
          );

          if (details) {
            setGameDetails((prev) => ({ ...prev, [gameName]: details }));
            processedNames.current.add(gameName); // Mark name as processed
          }
        } catch (error) {
          console.error(`Failed to fetch details for ${gameName}:`, error);
        }
      }
    };

    fetchDetailsForNewDownloads();
  }, [downloads]); // Run whenever downloads update

  // Function to drop a torrent
  const handleDropTorrent = async (name: string) => {
    try {
      await invoke("drop_torrent", { torrentFilePath: name });
      console.log(`Dropped torrent: ${name}`);
      setDownloads((prev) => prev.filter((download) => download.name !== name)); // Remove from list
    } catch (error) {
      console.error(`Failed to drop torrent ${name}:`, error);
    }
  };

  return (
    <div className="p-6">
      <h1 className="text-3xl mb-6 font-semibold">Downloads</h1>
      {downloads.length === 0 ? (
        <p className="text-gray-400">No active downloads.</p>
      ) : (
        <ul className="space-y-6">
          {downloads.map((download, index) => (
            <li
              key={index}
              className="flex flex-row items-start p-4 rounded-lg shadow-md"
            >
              {/* Game Image */}
              {gameDetails[download.name] && (
                <div className="flex-shrink-0 mr-4 w-32 h-32">
                  <img
                    src={gameDetails[download.name].header_image}
                    alt={`${download.name} cover`}
                    className="w-full h-full rounded-lg object-cover"
                  />
                </div>
              )}

              {/* Content Area */}
              <div className="flex-1 flex flex-col">
                <h2 className="text-lg font-medium text-gray-200 mb-4">
                  {download.name}
                </h2>

                {/* Combined Progress Bar */}
                <div className="relative bg-neutral-800 rounded h-6 mb-4">
                  <div
                    className="bg-green-500 h-6 rounded-l"
                    style={{
                      width: `${download.progress * 85 + download.extract_progress * 15}%`,
                    }}
                  ></div>
                  <div className="absolute right-2 top-1/2 transform -translate-y-1/2 text-sm text-gray-100 font-medium">
                    {Math.round(
                      download.progress * 85 + download.extract_progress * 15,
                    )}
                    %
                  </div>
                </div>

                {/* Speed, Peers */}
                <div className="flex justify-between items-center mt-2 text-sm text-gray-400">
                  <div className="flex items-center">
                    <span>
                      <i className="fas fa-person mr-1"></i>{" "}
                      {download.peers_connected}
                    </span>
                  </div>
                  <div className="text-right">
                    {download.speed_mb_ps.toFixed(2)} MB/s
                  </div>
                </div>

                {/* Downloaded and Total Size */}
                <div className="flex justify-between items-center mt-2 text-sm text-gray-400">
                  <span>
                    Downloaded:{" "}
                    {(download.downloaded / 1024 / 1024 || 0).toFixed(2)} MB
                  </span>
                  <span>
                    Total: {(download.total_size / 1024 / 1024 || 0).toFixed(2)}{" "}
                    MB
                  </span>
                </div>
                {/* ETA */}
                <div className="flex justify-between items-center mt-2 text-sm text-gray-400">
                  <span>ETA:</span>
                  <span>{download.eta}</span>
                </div>

                {/* Drop Torrent Button */}
                <button
                  onClick={() => handleDropTorrent(download.name)}
                  className=" text-red-500 hover:text-red-700"
                  title="Remove Download"
                >
                  <i className="fas fa-times"></i>
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

export default Downloads;
