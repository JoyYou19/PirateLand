import SidePanel from "./SidePanel";
import GameList from "./GameList";
import GameDetails from "./GameDetails";
import { BrowserRouter as Router, Routes, Route } from "react-router-dom";
import Downloads from "./Downloads";
import InstalledGames from "./Library";
import { useState } from "react";
import LogViewer from "./LogViewer";
//import { invoke } from "@tauri-apps/api/core";
//import { useEffect } from "react";

function App() {
  const [showLogs, setShowLogs] = useState(false);
  return (
    <Router>

      <LogViewer isOpen={showLogs} onClose={() => setShowLogs(false)} />
      <main className="bg-neutral-950 min-h-screen text-white grid grid-cols-[250px,1fr]">
        {/* Sidebar */}
        <div className="sticky top-0 h-screen">
          <SidePanel setShowLogs={setShowLogs}/>
        </div>

        {/* Main Content Area */}
        <div className="overflow-hidden">
          <div className="max-h-screen overflow-auto">
            <Routes>
              {/* Game List */}
              <Route path="/" element={<GameList />} />

              {/* Game Details */}
              <Route path="/game/:name" element={<GameDetails />} />

              {/* Downloads */}
              <Route path="/downloads" element={<Downloads />} />

              {/* Downloads */}
              <Route path="/library" element={<InstalledGames />} />
            </Routes>
          </div>
        </div>
      </main>
    </Router>
  );
}

export default App;
