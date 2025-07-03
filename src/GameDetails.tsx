import Skeleton from "react-loading-skeleton";
import "react-loading-skeleton/dist/skeleton.css";
import { useLocation, useParams } from "react-router-dom";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import Screenshots from "./Screenshots";
import { LazyLoadImage } from "react-lazy-load-image-component";
import "react-lazy-load-image-component/src/effects/blur.css";

interface PriceOverview {
  currency?: string;
  final_formatted?: string;
}

interface Screenshot {
  id?: number;
  path_full?: string;
  path_thumbnail?: string;
}

interface Genre {
  id?: string;
  description?: string;
}

interface PCRequirements {
  minimum?: string;
  recommended?: string;
}

interface GameDetailsData {
  name?: string;
  short_description?: string;
  header_image?: string;
  developers?: string[];
  publishers?: string[];
  price_overview?: PriceOverview;
  detailed_description?: string;
  about_the_game?: string;
  screenshots?: Screenshot[];
  genres?: Genre[];
  pc_requirements?: PCRequirements;
}

const GameDetails = () => {
  const { name } = useParams<{ name: string }>();
  const [gameDetails, setGameDetails] = useState<GameDetailsData | null>(null);
  const [loading, setLoading] = useState(true); // Track loading state
  const [defenderExcluded, setDefenderExcluded] = useState<boolean>(true);
  const [status, setStatus] = useState("Install"); // Tracks button text
  const location = useLocation();
  const [source, setSource] = useState<string>();
  const [gameUrl, setGameUrl] = useState("");

  const handleDownload = async (gameTitle: string) => {
    setStatus("Downloading...");

    try {
      if (source === "online-fix") {
        const result = await invoke<string>("download_torrent", { gameTitle });
        // If successful, set a success message
        console.log("Download result:", result);
      } else if (source === "igggames") {
        console.log(gameUrl);
        const result = await invoke<string>("download_igggames", {
          url: gameUrl,
          gameTitle: gameTitle,
        });
        console.log("PCGames download result:", result);
      }
      setStatus("Download Started");
    } catch (error) {
      console.error("Download error:", error);

      // If it fails, set a failure message
      setStatus("Failed");
    } finally {
      // Reset loading state after action completes
      setLoading(false);
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

  useEffect(() => {
    if (location.state) {
      setSource(location.state.source);
      setGameUrl(location.state.url);
    }
  }, [location]);

  useEffect(() => {
    const fetchDetails = async () => {
      setLoading(true);
      try {
        const details: GameDetailsData | null = await invoke(
          "find_and_get_game_details",
          { query: name },
        );
        setGameDetails(details);
      } catch (error) {
        console.error("Failed to fetch game details:", error);
      } finally {
        setLoading(false);
      }
    };

    fetchDetails();
  }, [name]);

  // Handle the case where `gameDetails` is null
  if (loading) {
    return (
      <div className="bg-neutral-950 text-white min-h-screen">
        <div className="max-w-7xl mx-auto px-8 py-12 grid grid-cols-1 lg:grid-cols-3 gap-12">
          {/* Skeleton Placeholders */}
          <div className="lg:col-span-2">
            <Skeleton
              height={40}
              width="60%"
              className="mb-4"
              baseColor="#2e2e2e"
              highlightColor="#3e3e3e"
            />
            <Skeleton
              height={300}
              className="mb-4"
              baseColor="#2e2e2e"
              highlightColor="#3e3e3e"
            />
            <Skeleton
              count={5}
              className="mb-4"
              baseColor="#2e2e2e"
              highlightColor="#3e3e3e"
            />
          </div>
          <div className="lg:col-span-1">
            <Skeleton
              height={250}
              className="mb-6"
              baseColor="#2e2e2e"
              highlightColor="#3e3e3e"
            />
            <Skeleton
              height={40}
              width="80%"
              className="mb-4"
              baseColor="#2e2e2e"
              highlightColor="#3e3e3e"
            />
            <Skeleton count={3} baseColor="#2e2e2e" highlightColor="#3e3e3e" />
          </div>
        </div>
      </div>
    );
  }

  if (!gameDetails) {
    return (
      <p className="text-center text-gray-400">Failed to load game details.</p>
    );
  }

  return (
    <div className="bg-neutral-950 text-white min-h-screen">
      <div className="max-w-7xl mx-auto px-8 py-12 grid grid-cols-1 lg:grid-cols-3 gap-12">
        {/* Left Column */}
        <div className="lg:col-span-2">
          {/* Title */}
          {gameDetails.name && (
            <h1 className="text-5xl font-bold mb-4">{gameDetails.name}</h1>
          )}

          {/* Screenshots */}
          {gameDetails.screenshots ? (
            <Screenshots gameDetails={gameDetails} />
          ) : (
            <Skeleton height={300} className="mb-4" />
          )}

          {/* Detailed Description */}
          {gameDetails.detailed_description && (
            <div className="mb-6">
              <h2 className="font-bold text-2xl mb-4">About the Game</h2>
              <div
                className="text-gray-300 leading-relaxed"
                dangerouslySetInnerHTML={{
                  __html: gameDetails.detailed_description,
                }}
              />
            </div>
          )}
        </div>

        {/* Right Column */}
        <div className="lg:col-span-1">
          <div className="sticky top-8 bg-neutral-900 p-6 rounded-lg shadow-lg">
            {/* Header Image */}
            {loading ? (
              <Skeleton height={250} className="mb-6" />
            ) : (
              gameDetails?.header_image && (
                <LazyLoadImage
                  src={gameDetails.header_image}
                  alt={gameDetails.name}
                  effect="blur"
                  className="w-full rounded-lg mb-6"
                />
              )
            )}

            {/* Install Button */}
            {!defenderExcluded ? (
              <div className="text-center">
                {/* Warning Message */}
                <p className="text-red-500 text-sm mb-2">
                  Please re-run the app with administrator privileges to allow
                  the folder in Windows Defender.
                </p>

                {/* Locked Button */}
                <button
                  className="w-full py-3 bg-gray-600 text-white text-lg font-bold rounded-lg cursor-not-allowed flex items-center justify-center"
                  disabled
                >
                  <i className="fas fa-lock mr-2"></i>
                  Install
                </button>
              </div>
            ) : (
              <button
                onClick={() => handleDownload(name || "Unknown Game")}
                className="w-full py-3 bg-blue-600 text-white text-lg font-bold rounded-lg hover:bg-blue-500 transition mb-4 flex items-center justify-center"
                disabled={loading} // Disable while loading
              >
                {loading ? <Skeleton height={24} width={80} /> : status}
              </button>
            )}

            {/* Short Description */}
            {loading ? (
              <Skeleton count={3} className="mb-4" />
            ) : (
              gameDetails?.short_description && (
                <p className="text-gray-300 text-sm mb-6">
                  {gameDetails.short_description}
                </p>
              )
            )}

            {/* Developers */}
            {loading ? (
              <Skeleton height={20} count={2} className="mb-4" />
            ) : (
              gameDetails?.developers && (
                <div className="mb-4">
                  <h3 className="font-bold text-lg mb-2">Developers</h3>
                  <ul>
                    {gameDetails.developers.map((dev) => (
                      <li key={dev} className="text-gray-300">
                        {dev}
                      </li>
                    ))}
                  </ul>
                </div>
              )
            )}

            {/* Publishers */}
            {gameDetails.publishers && (
              <div className="mb-4">
                <h3 className="font-bold text-lg mb-2">Publishers</h3>
                <ul>
                  {gameDetails.publishers.map((pub) => (
                    <li key={pub} className="text-gray-300">
                      {pub}
                    </li>
                  ))}
                </ul>
              </div>
            )}

            {/* Genres */}
            {gameDetails.genres && gameDetails.genres.length > 0 && (
              <div className="mb-4">
                <h3 className="font-bold text-lg mb-2">Genres</h3>
                <ul className="flex flex-wrap gap-2">
                  {gameDetails.genres.map((genre) => (
                    <li
                      key={genre.id}
                      className="bg-gray-800 text-gray-300 px-3 py-1 rounded-lg"
                    >
                      {genre.description}
                    </li>
                  ))}
                </ul>
              </div>
            )}

            {/* Price */}
            {loading ? (
              <Skeleton height={20} width="50%" />
            ) : (
              gameDetails?.price_overview && (
                <div className="text-lg font-bold flex items-center space-x-2">
                  <span>Price:</span>
                  <span
                    className={`px-4 py-2 rounded-xl shadow-lg text-white ${
                      gameDetails.price_overview.final_formatted
                        ? getPriceColor(
                            gameDetails.price_overview.final_formatted,
                          )
                        : "bg-gray-500"
                    }`}
                  >
                    {gameDetails.price_overview.final_formatted}
                  </span>
                </div>
              )
            )}

            {/* PC Requirements */}
            {gameDetails.pc_requirements && (
              <div className="mt-6">
                <h3 className="font-bold text-lg mb-3 flex items-center">
                  <i className="fas fa-desktop mr-2 text-blue-400" />
                  System Requirements
                </h3>

                {gameDetails.pc_requirements.minimum && (
                  <div className="mb-4">
                    <h4 className="font-semibold text-gray-400 mb-1 flex items-center">
                      <i className="fas fa-microchip mr-2 text-sm" />
                      Minimum
                    </h4>
                    <div
                      className="text-gray-300 text-sm bg-neutral-800 p-3 rounded-lg"
                      dangerouslySetInnerHTML={{
                        __html: gameDetails.pc_requirements.minimum,
                      }}
                    />
                  </div>
                )}

                {gameDetails.pc_requirements.recommended && (
                  <div>
                    <h4 className="font-semibold text-gray-400 mb-1 flex items-center">
                      <i className="fas fa-tachometer-alt mr-2 text-sm" />
                      Recommended
                    </h4>
                    <div
                      className="text-gray-300 text-sm bg-neutral-800 p-3 rounded-lg"
                      dangerouslySetInnerHTML={{
                        __html: gameDetails.pc_requirements.recommended,
                      }}
                    />
                  </div>
                )}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};

export default GameDetails;

const getPriceColor = (price: string) => {
  // Replace commas with dots for decimal parsing and remove non-numeric characters (except dots)
  const formattedPrice = price.replace(",", ".").replace(/[^0-9.]/g, "");
  const numericPrice = parseFloat(formattedPrice);

  if (isNaN(numericPrice)) {
    return "bg-gray-500"; // Default color for invalid prices
  }

  if (numericPrice <= 20) {
    return "bg-green-600"; // Green for low prices
  } else if (numericPrice <= 30) {
    return "bg-yellow-600"; // Yellow for medium prices
  } else if (numericPrice <= 60) {
    return "bg-red-600"; // Orange for higher prices
  } else {
    return "bg-purple-600"; // Red for expensive prices
  }
};
