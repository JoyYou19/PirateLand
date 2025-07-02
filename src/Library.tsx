import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface InstalledGame {
  name: string;
  path: string;
}

interface GameDetailsData {
  header_image?: string;
}

function InstalledGames() {
  const [games, setGames] = useState<InstalledGame[]>([]);
  const [gameDetails, setGameDetails] = useState<
    Record<string, GameDetailsData>
  >({});
  const processedNames = useRef<Set<string>>(new Set()); // Track already processed names
  const [showConfirmation, setShowConfirmation] = useState(false);
  const [gameToUninstall, setGameToUninstall] = useState<string | null>(null);
  // Fetch the list of installed games
  useEffect(() => {
    const fetchInstalledGames = async () => {
      try {
        const response: InstalledGame[] = await invoke("get_installed_games");
        setGames(response);
      } catch (error) {
        console.error("Failed to fetch installed games:", error);
      }
    };

    fetchInstalledGames();
  }, []);

  const handleOpenFolder = async (path: string, gameTitle: string) => {
    try {
      await invoke("open_folder", { folderPath: path });
      await invoke("update_recent_games", { name: gameTitle, path: path });
      console.log(`Opened folder: ${path}`);
    } catch (error) {
      console.error(`Failed to open folder: ${error}`);
    }
  };

  const handleUninstall = async (path: string) => {
    try {
      // Call the Rust command to uninstall the game
      await invoke("uninstall_game", { gamePath: path });

      // Remove the game entry from the games list
      setGames((prevGames) => prevGames.filter((game) => game.path !== path));
    } catch (error) {
      console.error(`Failed to uninstall game: ${error}`);
    } finally {
      // Close the confirmation dialog after uninstalling
      setShowConfirmation(false);
    }
  };

  const handleCancelUninstall = () => {
    setShowConfirmation(false); // Close the dialog if canceled
  };

  // Fetch game details for new downloads
  useEffect(() => {
    const fetchDetailsForNewDownloads = async () => {
      for (const game of games) {
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
  }, [games]); // Run whenever downloads update

  return (
    <div className="p-6 overflow-x-hidden">
      <h1 className="text-3xl mb-6 font-semibold">Installed Games</h1>
      {games.length === 0 ? (
        <p className="text-gray-400">No games installed.</p>
      ) : (
        <div className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-6">
          {games.map((game, index) => (
            <div
              key={index}
              className="relative flex flex-col items-center rounded-xl overflow-visible group"
            >
              <div
                onClick={() => handleOpenFolder(`${game.path}`, game.name)}
                className="cursor-pointer w-full relative"
              >
                <div className="relative">
                  {/* Only render images if gameDetails[game.name] and header_image exist */}
                  {gameDetails[game.name]?.header_image && (
                    <>
                      {/* Glow (blurred version of the image) */}
                      <img
                        src={gameDetails[game.name].header_image}
                        alt={game.name}
                        className="absolute inset-0 scale-100 object-cover rounded-[2rem] blur-xl opacity-0 group-hover:opacity-100 transition-opacity duration-300"
                      />

                      {/* Main Image */}
                      <img
                        src={gameDetails[game.name].header_image}
                        alt={game.name}
                        className="relative w-full object-cover rounded-xl transition-transform duration-300"
                      />

                      {/* Border */}
                      <div className="absolute -inset-[10px] rounded-3xl border-4 border-white opacity-0 group-hover:opacity-80 transition-opacity duration-300"></div>
                    </>
                  )}
                </div>

                {/* Title */}
                <h2 className="mt-2 text-white text-sm font-semibold text-center">
                  {game.name}
                </h2>
              </div>
              {/* Hover Button */}
              <button
                onClick={() => {
                  setGameToUninstall(game.path); // Set the game to uninstall
                  setShowConfirmation(true); // Show the confirmation dialog
                }}
                className="absolute bottom-4 right-4 px-4 py-2 bg-red-600 text-white text-sm rounded-lg opacity-0 group-hover:opacity-100 transition-opacity duration-300"
              >
                Uninstall
              </button>
            </div>
          ))}
        </div>
      )}

      {/* Confirmation Dialog */}
      {showConfirmation && gameToUninstall && (
        <div className="fixed inset-0 bg-black bg-opacity-50 flex justify-center items-center">
          <div className="bg-neutral-900 p-6 rounded-lg w-96">
            <h3 className="text-lg font-semibold">
              Are you sure you want to uninstall this game?
            </h3>
            <div className="mt-4 flex justify-end space-x-4">
              <button
                onClick={handleCancelUninstall}
                className="px-4 py-2 bg-neutral-600 rounded-md"
              >
                Cancel
              </button>
              <button
                onClick={() => handleUninstall(gameToUninstall)}
                className="px-4 py-2  bg-red-600 text-white rounded-md"
              >
                <i className="fas fa-trash" />
                Uninstall
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default InstalledGames;
