import  { useEffect, useRef, useState } from "react";
import logo from "./assets/pirate_land_icon.svg";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";

interface RecentGame {
  name: string;
  path: string;
}

interface GameDetailsData {
  header_image?: string;
}

const SidePanel = ({ setShowLogs }: { setShowLogs: (show: boolean) => void }) => {
  const navigate = useNavigate();
  const [recentGames, setRecentGames] = useState<RecentGame[]>([]);
  const [defenderExcluded, setDefenderExcluded] = useState<boolean>(true);
  const processedNames = useRef<Set<string>>(new Set()); // Track already processed names
  const [gameDetails, setGameDetails] = useState<
    Record<string, GameDetailsData>
  >({});

  const handleDownloads = () => {
    navigate(`/downloads`);
  };

  const handleStore = () => {
    navigate(`/`);
  };

  const handleLibrary = () => {
    navigate(`/library`);
  };

  const handleOpenFolder = async (path: string) => {
    try {
      await invoke("open_folder", { folderPath: path });
      console.log(`Opened folder: ${path}`);
    } catch (error) {
      console.error(`Failed to open folder: ${error}`);
    }
  };

  // Fetch game details for new downloads
  useEffect(() => {
    const fetchDetailsForNewDownloads = async () => {
      for (const game of recentGames) {
        const gameName = game.name;

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
            console.log(details);
            setGameDetails((prev) => ({ ...prev, [gameName]: details }));
            processedNames.current.add(gameName); // Mark name as processed
          }
        } catch (error) {
          console.error(`Failed to fetch details for ${gameName}:`, error);
        }
      }
    };

    fetchDetailsForNewDownloads();
  }, [recentGames]); // Run whenever downloads update

  const handleExclude = async () => {
    try {
      const result = await invoke("exclude_folder_in_defender");
      await invoke("set_defender_exclusion_status", { status: true });
      setDefenderExcluded(true);
      alert(result); // Show success message
    } catch (error) {
      console.error("Error excluding folder in Defender:", error);
      alert(
        "Failed to exclude the folder in Windows Defender, missing Admin Priviliges.",
      );
    }
  };

  useEffect(() => {
    const checkExclusionStatus = async () => {
      try {
        const status: boolean = await invoke("check_defender_exclusion");
        setDefenderExcluded(status);
      } catch (error) {
        console.error("Failed to check Defender exclusion status:", error);
      }
    };

    checkExclusionStatus();
  }, []);

  // Fetch recent games on mount
  useEffect(() => {
    const fetchRecentGames = async () => {
      try {
        const games: RecentGame[] = await invoke("get_recent_games");
        setRecentGames(games);
      } catch (error) {
        console.error("Failed to fetch recent games:", error);
      }
    };

    fetchRecentGames();
  }, []);

  return (
    <aside className="bg-neutral-950 text-white min-h-screen p-6 flex flex-col">
      {/* Logo Section */}
      <div className="flex items-center justify-center mb-8">
        <img src={logo} alt="Pirate Land Logo" className="w-16 h-16" />
      </div>

      {/* Menu Items */}
      <nav className="flex-grow">
        <ul className="space-y-6">
          <li
            className="flex items-center gap-4 p-4 rounded-lg hover:bg-neutral-800 cursor-pointer"
            onClick={handleStore}
          >
            <i className="fas fa-store text-white text-lg flex items-center justify-center"></i>
            <span className="text-lg">Store</span>
          </li>
          <li
            className="flex items-center gap-4 p-4 rounded-lg hover:bg-neutral-800 cursor-pointer"
            onClick={handleLibrary}
          >
            <i className="fas fa-list text-white text-lg flex items-center justify-center"></i>
            <span className="text-lg">Library</span>
          </li>
          <li
            className="flex items-center gap-4 p-4 rounded-lg hover:bg-neutral-800 cursor-pointer"
            onClick={handleDownloads}
          >
            <i className="fas fa-download text-white text-lg flex items-center justify-center"></i>
            <span className="text-lg">Downloads</span>
          </li>
        </ul>
      </nav>

      {
        /* Quick Launch Section */
        <div className="mt-6">
          <h2 className="text-sm font-bold uppercase text-gray-400 mb-4">
            Quick Launch
          </h2>
          {recentGames.length === 0 ? (
            <p className="text-gray-500 text-sm">No recent games.</p>
          ) : (
            <div className="space-y-4">
              {recentGames.map((game, index) => (
                <div
                  key={index}
                  className="flex items-center gap-4 cursor-pointer"
                  onClick={() => {
                    // Handle quick launch action here, like opening the game folder
                    handleOpenFolder(game.path);
                    console.log(`Launching: ${game.name}`);
                  }}
                >
                  {gameDetails[game.name]?.header_image && (
                    <img
                      src={gameDetails[game.name].header_image}
                      alt={game.name}
                      className="w-10 h-10 rounded-md object-cover"
                    />
                  )}
                  <span>{game.name}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      }

      {/* Defender Warning */}
      {!defenderExcluded && (
        <div className="mt-6 text-red-500 text-sm">
          <p>
            Please re-run this application with administrator privileges to
            allow the download folder in Windows Defender.
          </p>
          <button
            onClick={handleExclude}
            className="mt-2 bg-red-700 px-4 py-2 rounded-md hover:bg-red-600"
          >
            Allow Folder in Defender
          </button>
        </div>
      )}
    </aside>
  );
};

export default SidePanel;
