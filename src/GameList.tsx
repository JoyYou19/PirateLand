import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useNavigate } from "react-router-dom";

type Game = {
  title: string;
  link: string;
  image: string;
  release_date?: string;
  modes?: string;
  views?: string;
};

type SteamGame = {
  appid: string;
  name: string;
  header_image: string;
  recommendations: number;
  positive: number;
  negative: number;
};

type GameCategory = "onlinefix" | "most_recommended" | "best_reviewed";

// Unified game type for display
type DisplayGame =
  | { type: "onlinefix"; data: Game }
  | { type: "steam"; data: SteamGame };

type SearchResult = {
  title: string;
  url: string;
  source: string;
};

function getProxiedImageUrl(originalImageUrl: string): string {
  return `http://127.0.0.1:3030/proxy?url=${encodeURIComponent(originalImageUrl)}`;
}

export default function GameList() {
  const [games, setGames] = useState<DisplayGame[]>([]);
  const [page, setPage] = useState(1); // Current page number
  const [loading, setLoading] = useState(false); // Prevent multiple fetches
  const [error, setError] = useState("");
  const [hasMore, setHasMore] = useState(true); // Track if there are more pages to load
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<SearchResult[]>([]);
  const [showResults, setShowResults] = useState(false); // To toggle visibility of results
  const [activeCategory, setActiveCategory] =
    useState<GameCategory>("onlinefix");
  const searchContainerRef = useRef<HTMLDivElement>(null); // Reference for the search container
  const navigate = useNavigate();

  useEffect(() => {
    // Reset state when category changes
    setGames([]);
    setPage(1);
    setHasMore(true);
    fetchGames(1);
    checkAuthentication();
  }, [activeCategory]);

  useEffect(() => {
    fetchGames(page);
    checkAuthentication();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [page]);

  // Debounced search function
  const handleSearchBar = async (query: string) => {
    if (!query || query.length < 3) return; // Skip short queries
    setLoading(true);
    try {
      const onlineFixResults: SearchResult[] = await invoke(
        "search_online_fix",
        {
          query,
        },
      );

      console.log("online fix results: ", onlineFixResults);

      const iggResult: SearchResult[] = await invoke("search_igggames", {
        query,
      });
      const combinedResults = [
        ...onlineFixResults.map((r) => ({ ...r, source: "online-fix" })),
        ...iggResult.map((r) => ({ ...r, source: "igggames" })),
      ];

      setSearchResults(combinedResults);
      setShowResults(true);
    } catch (error) {
      console.error("Search failed:", error);
    } finally {
      setLoading(false);
    }
  };

  // Debounce user input
  const handleInputChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    const value = event.target.value;
    setSearchQuery(value);

    if (value.length >= 3) {
      setTimeout(() => handleSearchBar(value), 300); // 300ms debounce
    } else {
      setShowResults(false); // Hide results for short or empty queries
    }
  };

  // Close results when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        searchContainerRef.current &&
        !searchContainerRef.current.contains(event.target as Node)
      ) {
        setShowResults(false); // Hide results when clicking outside
      }
    };

    document.addEventListener("mousedown", handleClickOutside);
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, []);

  async function checkAuthentication() {
    try {
      const cfClearance =
        "rcMUAtMC9AxOrDyGRfCJBWjh.aLlWjd3BjW5fuW8Epc-1732107386-1.2.1.1-y3wJv.FPErun1wtdVvDNQi5hjKnBOL58Fe.5c3nEuD_06WOkXa_RuBrwyHo3c.MCusNXBlXDpgwLOvORlmYEbNEbR8JfuLlxkpo0D7xl1iO6Ps_OQ8ZpUTSdP0FuqF_usU2Toji_yID8xXBb1jez61T.qBWjTBXvad1SCY4qtq4IvOkFofXHtxeaZIowWR3xvGMShyanVlmaI_zkUtM1WOLPverTWUj7m3skLSePWV5u4Ai3bpAZe9Z5ueengDL3.HL3jCRtlvDJh56pNjBqA8gLBQbOPELFabrONve.4gvyg53YmP5RLUz.tNrbmvq5dEzHZ_gSYW2m4oRyuj__Mw"; // Replace with the actual cookie value
      const phpSessId = "7g1qocjlnh2p62t0ekl6b4ft4n"; // Replace with the actual cookie value

      // Invoke the `authenticate` Tauri command
      const response = await invoke("authenticate", {
        cfClearance: cfClearance,
        phpSessid: phpSessId,
      });

      console.log(response); // Should log "Authentication successful!" if successful
    } catch (error) {
      console.error("Error checking authentication:", error);
    }
  }

  // Update the fetchGames function to include explicit types
  async function fetchGames(page: number) {
    if (loading || !hasMore) return;
    setLoading(true);

    try {
      if (activeCategory === "onlinefix") {
        const fetchedGames = await invoke<Game[]>("fetch_games", {
          page,
        });

        if (fetchedGames.length === 0) {
          setHasMore(false);
        } else {
          // Explicit type casting for onlinefix games
          const newGames: DisplayGame[] = fetchedGames.map(
            (game) =>
              ({
                type: "onlinefix",
                data: game,
              }) as DisplayGame,
          );

          setGames((prev) => [...prev, ...newGames]);
        }
      } else {
        const steamCategory =
          activeCategory === "most_recommended"
            ? "most_recommended"
            : "best_reviewed";

        const fetchedGames = await invoke<SteamGame[]>("fetch_games_index", {
          category: steamCategory,
          page: page - 1,
          pageSize: 20,
        });

        if (fetchedGames.length === 0) {
          setHasMore(false);
        } else {
          // Explicit type casting for steam games
          const newGames: DisplayGame[] = fetchedGames.map(
            (game) =>
              ({
                type: "steam",
                data: game,
              }) as DisplayGame,
          );

          setGames((prev) => [...prev, ...newGames]);
        }
      }
    } catch (err) {
      console.error("Failed to fetch games:", err);
      setError("Failed to fetch games. Please try again.");
    } finally {
      setLoading(false);
    }
  }

  const handleScroll = (e: Event) => {
    const target = e.target as HTMLElement;

    // Check if the user has scrolled near the bottom of the scrollable container
    const scrollPosition = target.scrollTop + target.clientHeight;
    const threshold = target.scrollHeight - 500;

    if (scrollPosition >= threshold && !loading && hasMore) {
      setPage((prevPage) => prevPage + 1); // Increment page number
    }
  };

  const handleSearch = (result: SearchResult) => {
    navigate(`/game/${encodeURIComponent(result.title)}`, {
      state: { source: result.source, url: result.url },
    });
  };

  const handleGameClick = async (game: DisplayGame) => {
    if (game.type === "onlinefix") {
      navigate(`/game/${encodeURIComponent(game.data.title)}`, {
        state: {
          source: "online-fix",
          url: game.data.link,
        },
      });
    } else {
      try {
        // Search IGGGames for the Steam game name
        const iggResults = await invoke<SearchResult[]>("search_igggames", {
          query: game.data.name,
        });

        if (iggResults.length > 0) {
          // Take the first result
          const firstResult = iggResults[0];
          navigate(`/game/${encodeURIComponent(firstResult.title)}`, {
            state: {
              source: "igggames",
              url: firstResult.url,
            },
          });
        } else {
          // Fallback to Steam page if no IGG result
          open(`https://store.steampowered.com/app/${game.data.appid}`);
        }
      } catch (error) {
        console.error("Failed to search IGGGames:", error);
        // Fallback to Steam page on error
        open(`https://store.steampowered.com/app/${game.data.appid}`);
      }
    }
  };

  useEffect(() => {
    const scrollableContainer = document.querySelector(".overflow-auto");
    if (scrollableContainer) {
      scrollableContainer.addEventListener("scroll", handleScroll);
    }
    return () => {
      if (scrollableContainer) {
        scrollableContainer.removeEventListener("scroll", handleScroll);
      }
    };
  }, [loading, hasMore]); // Add `loading` and `hasMore` to dependency array

  if (error) {
    return <p className="text-center text-red-500">{error}</p>;
  }

  return (
    <div className="bg-neutral-900 min-h-screen">
      {/* Header with categories and search */}
      <div className="sticky top-0 z-20 bg-neutral-900 border-b border-neutral-700 px-8 py-4">
        <div className="flex items-center justify-between">
          {/* Categories on the left */}
          <div className="flex space-x-1">
            <button
              className={`px-4 py-2 rounded-lg transition-all bg-neutral-900 ${
                activeCategory === "onlinefix"
                  ? "bg-neutral-800 text-blue-400 font-semibold"
                  : "text-neutral-400 hover:bg-neutral-800 hover:text-white"
              }`}
              onClick={() => setActiveCategory("onlinefix")}
            >
              OnlineFix
            </button>
            <button
              className={`px-4 py-2 rounded-lg transition-all bg-neutral-900  ${
                activeCategory === "most_recommended"
                  ? "bg-neutral-800 text-blue-400 font-semibold"
                  : "text-neutral-400 hover:bg-neutral-800 hover:text-white"
              }`}
              onClick={() => setActiveCategory("most_recommended")}
            >
              Most Recommended
            </button>
            <button
              className={`px-4 py-2 rounded-lg transition-all bg-neutral-900  ${
                activeCategory === "best_reviewed"
                  ? "bg-neutral-800 text-blue-400 font-semibold"
                  : "text-neutral-400 hover:bg-neutral-800 hover:text-white"
              }`}
              onClick={() => setActiveCategory("best_reviewed")}
            >
              Best Reviewed
            </button>
          </div>

          {/* Search Bar */}
          <div ref={searchContainerRef} className="sticky top-0 pl-5 z-10">
            <div className="relative max-w-md mx-auto">
              {/* Search Icon */}
              <svg
                xmlns="http://www.w3.org/2000/svg"
                fill="none"
                viewBox="0 0 24 24"
                strokeWidth={2}
                stroke="currentColor"
                className="absolute left-3 top-2.5 w-5 h-5 text-gray-400"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M21 21l-4.35-4.35m2.1-5.4a7.5 7.5 0 11-15 0 7.5 7.5 0 0115 0z"
                />
              </svg>

              {/* Input Field */}
              <input
                type="text"
                placeholder="Search store"
                value={searchQuery}
                onChange={handleInputChange}
                className="w-full pl-10 pr-4 py-2.5 rounded-full bg-neutral-800 text-gray-300 placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500 transition"
              />
            </div>

            {/* Search results dropdown */}
            {showResults && searchResults.length > 0 && (
              <div className="absolute top-12 right-0 bg-neutral-800 rounded-lg shadow-xl w-full z-50 overflow-hidden">
                <div className="py-2">
                  {loading && (
                    <p className="px-4 py-2 text-neutral-400">Searching...</p>
                  )}
                  {!loading &&
                    searchResults.map((result, index) => (
                      <div
                        key={index}
                        onClick={() => {
                          setSearchQuery(result.title);
                          handleSearch(result);
                          setShowResults(false);
                        }}
                        className="px-4 py-3 cursor-pointer hover:bg-neutral-700 transition-colors"
                      >
                        <div className="flex justify-between items-center">
                          <span className="text-neutral-200 truncate">
                            {result.title}
                          </span>
                          <span
                            className={`text-xs px-2 py-1 rounded ${
                              result.source === "online-fix"
                                ? "bg-blue-500"
                                : "bg-green-500"
                            }`}
                          >
                            {result.source === "online-fix"
                              ? "OnlineFix"
                              : "PCGames"}
                          </span>
                        </div>
                      </div>
                    ))}
                </div>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Game Grid */}
      <div className="p-8 grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-6">
        {games.map((game, index) => (
          <GameCard
            key={`${game.type}-${index}`}
            game={game}
            onClick={() => handleGameClick(game)}
          />
        ))}
        {loading && (
          <div className="col-span-full flex justify-center py-8">
            <div className="animate-spin rounded-full h-10 w-10 border-t-2 border-b-2 border-blue-500"></div>
          </div>
        )}
        {!hasMore && !loading && games.length > 0 && (
          <p className="col-span-full text-center text-neutral-500 py-4">
            No more games to load
          </p>
        )}
      </div>
    </div>
  );
}

// Game Card Component
function GameCard({
  game,
  onClick,
}: {
  game: DisplayGame;
  onClick: () => void;
}) {
  const imageUrl =
    game.type === "onlinefix" ? game.data.image : game.data.header_image;

  const title = game.type === "onlinefix" ? game.data.title : game.data.name;

  return (
    <div className="relative flex flex-col items-center rounded-xl overflow-visible group">
      <div onClick={onClick} className="cursor-pointer w-full relative">
        <div className="relative">
          {/* Glow effect */}
          <img
            src={getProxiedImageUrl(imageUrl)}
            alt={title}
            className="absolute inset-0 scale-125 object-cover rounded-[2rem] blur-2xl opacity-0 group-hover:opacity-100 transition-opacity duration-300"
          />

          {/* Main Image */}
          <img
            src={getProxiedImageUrl(imageUrl)}
            alt={title}
            className="relative w-full aspect-[16/9] object-cover rounded-xl transition-transform duration-300"
          />

          {/* Border */}
          <div className="absolute -inset-[10px] rounded-3xl border-4 border-white opacity-0 group-hover:opacity-80 transition-opacity duration-300"></div>
        </div>

        {/* Title */}
        <h2 className="mt-2 text-white text-sm font-semibold text-center line-clamp-2 h-10">
          {title}
        </h2>
      </div>
    </div>
  );
}
