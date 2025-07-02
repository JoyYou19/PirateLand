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

function getProxiedImageUrl(originalImageUrl: string): string {
  return `http://127.0.0.1:3030/proxy?url=${encodeURIComponent(originalImageUrl)}`;
}

export default function GameList() {
  const [games, setGames] = useState<Game[]>([]);
  const [page, setPage] = useState(1); // Current page number
  const [loading, setLoading] = useState(false); // Prevent multiple fetches
  const [error, setError] = useState("");
  const [hasMore, setHasMore] = useState(true); // Track if there are more pages to load
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<string[]>([]);
  const [showResults, setShowResults] = useState(false); // To toggle visibility of results
  const searchContainerRef = useRef<HTMLDivElement>(null); // Reference for the search container
  const navigate = useNavigate();

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
      const response: string[] = await invoke("search_online_fix", {
        query,
      });
      setSearchResults(response); // Assuming response is a string array
      setShowResults(true); // Show results on successful search
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

  async function fetchGames(page: number) {
    if (loading || !hasMore) return; // Prevent multiple simultaneous fetches
    setLoading(true);
    try {
      const fetchedGames = (await invoke("fetch_games", { page })) as Game[];
      if (fetchedGames.length === 0) {
        setHasMore(false); // No more games to load
      } else {
        // Remove duplicates by title
        setGames((prevGames) => {
          const allGames = [...prevGames, ...fetchedGames];
          const uniqueGames = Array.from(
            new Map(allGames.map((game) => [game.title, game])).values(),
          );
          return uniqueGames;
        });
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

  const handleSearch = (gameName: string) => {
    console.log("trying to go to the next search object");
    navigate(`/game/${encodeURIComponent(gameName)}`);
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
    <div>
      {/* Search Bar */}
      <div ref={searchContainerRef} className="sticky top-0 pt-4 z-10">
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

        {/* Search Results */}
        {showResults && searchResults.length > 0 && (
          <div className="relative">
            <div className="absolute top-2 left-1/2 transform -translate-x-1/2 bg-neutral-900 rounded-lg shadow-lg p-4 w-80 z-50">
              {loading && <p className="text-gray-400">Loading...</p>}
              {!loading &&
                searchResults.map((result, index) => (
                  <div
                    key={index}
                    onClick={() => {
                      setSearchQuery(result); // Set search query on selection
                      handleSearch(result);
                      setShowResults(false); // Hide results on selection
                    }}
                    className="cursor-pointer p-2 hover:bg-neutral-700 rounded-md text-gray-300 text-center"
                  >
                    {result}
                  </div>
                ))}
            </div>
          </div>
        )}
      </div>

      <div className="p-8 grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-5 gap-6">
        {games.map((game, index) => (
          <div
            key={index}
            className="relative flex flex-col items-center rounded-xl overflow-visible group"
          >
            <div
              onClick={() => handleSearch(game.title)}
              className="cursor-pointer w-full relative"
            >
              <div className="relative">
                {/* Glow (blurred version of the image) */}
                <img
                  src={getProxiedImageUrl(game.image)}
                  alt={game.title}
                  className="absolute inset-0 scale-125 object-cover rounded-[2rem] blur-2xl opacity-0 group-hover:opacity-100 transition-opacity duration-300"
                />

                {/* Main Image */}
                <img
                  src={getProxiedImageUrl(game.image)}
                  alt={game.title}
                  className="relative w-full object-cover rounded-xl transition-transform duration-300"
                />

                {/* Border */}
                <div className="absolute -inset-[10px] rounded-3xl border-4 border-white opacity-0 group-hover:opacity-80 transition-opacity duration-300"></div>
              </div>

              {/* Title */}
              <h2 className="mt-2 text-white text-sm font-semibold text-center">
                {game.title}
              </h2>
            </div>
          </div>
        ))}
        {loading && <p className="text-center text-gray-400">Loading...</p>}
        {!hasMore && (
          <p className="text-center text-gray-500">No more games to load.</p>
        )}
      </div>
    </div>
  );
}
