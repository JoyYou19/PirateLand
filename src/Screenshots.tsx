import { useState } from "react";
import { LazyLoadImage } from "react-lazy-load-image-component";
import "react-lazy-load-image-component/src/effects/blur.css";

// Define types
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
}

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

interface ScreenshotsProps {
  gameDetails: GameDetailsData;
}

function Screenshots({ gameDetails }: ScreenshotsProps) {
  const [currentScreenshotIndex, setCurrentScreenshotIndex] =
    useState<number>(0);
  const [startThumbnailIndex, setStartThumbnailIndex] = useState<number>(0);

  // Guard clause for empty screenshots
  if (!gameDetails.screenshots || gameDetails.screenshots.length === 0) {
    return null; // Don't render anything if screenshots are unavailable
  }

  const screenshots = gameDetails.screenshots;
  const maxVisibleThumbnails = 6; // Show up to 6 thumbnails at a time

  // Function to handle main screenshot navigation
  const handleThumbnailClick = (index: number): void => {
    setCurrentScreenshotIndex(index);
  };

  const handlePrevClick = (): void => {
    setCurrentScreenshotIndex((prevIndex) =>
      prevIndex > 0 ? prevIndex - 1 : screenshots.length - 1,
    );
  };

  const handleNextClick = (): void => {
    setCurrentScreenshotIndex((prevIndex) =>
      prevIndex < screenshots.length - 1 ? prevIndex + 1 : 0,
    );
  };

  // Thumbnail carousel navigation
  const scrollThumbnailsLeft = (): void => {
    setStartThumbnailIndex((prevIndex) => Math.max(0, prevIndex - 1));
  };

  const scrollThumbnailsRight = (): void => {
    setStartThumbnailIndex((prevIndex) =>
      Math.min(screenshots.length - maxVisibleThumbnails, prevIndex + 1),
    );
  };

  // Determine visible thumbnails
  const visibleThumbnails = screenshots.slice(
    startThumbnailIndex,
    startThumbnailIndex + maxVisibleThumbnails,
  );

  return (
    <div className="mb-6">
      {/* Main Screenshot */}
      <div className="relative w-full mb-4 group">
        <LazyLoadImage
          src={
            screenshots[currentScreenshotIndex].path_full ||
            screenshots[currentScreenshotIndex].path_thumbnail
          }
          alt={`Screenshot ${screenshots[currentScreenshotIndex].id}`}
          effect="blur"
          className="w-full h-full object-cover rounded-lg shadow-lg"
        />

        {/* Left Arrow */}
        <button
          onClick={handlePrevClick}
          className="absolute top-1/2 left-2 transform -translate-y-1/2 bg-black bg-opacity-50 text-white p-2 rounded-full focus:outline-none hover:bg-opacity-75 opacity-0 group-hover:opacity-100 transition-opacity"
        >
          ❮
        </button>

        {/* Right Arrow */}
        <button
          onClick={handleNextClick}
          className="absolute top-1/2 right-2 transform -translate-y-1/2 bg-black bg-opacity-50 text-white p-2 rounded-full focus:outline-none hover:bg-opacity-75 opacity-0 group-hover:opacity-100 transition-opacity"
        >
          ❯
        </button>
      </div>

      {/* Thumbnails Carousel */}
      <div className="flex items-center gap-2">
        {/* Left Scroll Arrow */}
        <button
          onClick={scrollThumbnailsLeft}
          className="p-2 bg-black bg-opacity-50 text-white rounded-full hover:bg-opacity-75 focus:outline-none"
          disabled={startThumbnailIndex === 0}
        >
          ❮
        </button>

        {/* Visible Thumbnails */}
        <div className="flex gap-2 overflow-visible">
          {visibleThumbnails.map((screenshot, index) => {
            const actualIndex = startThumbnailIndex + index;
            return (
              <LazyLoadImage
                key={screenshot.id}
                src={screenshot.path_thumbnail || screenshot.path_full}
                alt={`Thumbnail ${screenshot.id}`}
                effect="blur"
                className={`w-28 h-18 object-cover rounded-lg shadow-lg cursor-pointer ${
                  actualIndex === currentScreenshotIndex
                    ? "ring-4 ring-gray-100"
                    : "hover:ring-2 hover:ring-gray-100"
                }`}
                onClick={() => handleThumbnailClick(actualIndex)}
              />
            );
          })}
        </div>

        {/* Right Scroll Arrow */}
        <button
          onClick={scrollThumbnailsRight}
          className="p-2 bg-black bg-opacity-50 text-white rounded-full hover:bg-opacity-75 focus:outline-none"
          disabled={
            startThumbnailIndex + maxVisibleThumbnails >= screenshots.length
          }
        >
          ❯
        </button>
      </div>
    </div>
  );
}

export default Screenshots;
