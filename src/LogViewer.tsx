import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface LogViewerProps {
  isOpen: boolean;
  onClose: () => void;
}

function LogViewer({ isOpen, onClose }: LogViewerProps) {
  const [logs, setLogs] = useState<string[]>([]);
  const [filter, setFilter] = useState("");
  const [autoScroll, setAutoScroll] = useState(true);
  const logsEndRef = useRef<HTMLDivElement>(null);
  const refreshIntervalRef = useRef<NodeJS.Timeout>();

  // Fetch logs periodically
  useEffect(() => {
    if (!isOpen) return;

    const fetchLogs = async () => {
      try {
        const logs: unknown = await invoke("get_go_server_logs");
        if (Array.isArray(logs)) {
          setLogs(logs.filter(line => typeof line === 'string'));
        }
      } catch (error) {
        console.error("Error fetching logs:", error);
      }
    };

    // Initial fetch
    fetchLogs();

    // Set up periodic refresh (every 2 seconds)
    refreshIntervalRef.current = setInterval(fetchLogs, 2000);

    return () => {
      if (refreshIntervalRef.current) {
        clearInterval(refreshIntervalRef.current);
      }
    };
  }, [isOpen]);

  // Auto-scroll to bottom when new logs arrive
  useEffect(() => {
    if (autoScroll && logsEndRef.current) {
      logsEndRef.current.scrollIntoView({ behavior: "auto" });
    }
  }, [logs, autoScroll]);

  // Filter logs based on search term
  const filteredLogs = logs.filter((log) =>
    log.toLowerCase().includes(filter.toLowerCase())
  );

  // Get icon for log type
  const getLogIcon = (log: string) => {
    if (log.includes("Error") || log.includes("error")) {
      return <i className="fas fa-times-circle text-red-500 mr-2" />;
    }
    if (log.includes("Warning") || log.includes("warning")) {
      return <i className="fas fa-exclamation-triangle text-yellow-500 mr-2" />;
    }
    if (log.includes("INFO") || log.includes("Info")) {
      return <i className="fas fa-info-circle text-blue-400 mr-2" />;
    }
    return <i className="fas fa-terminal text-gray-400 mr-2" />;
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black bg-opacity-90 flex items-center justify-center z-50 p-4">
      <div className="bg-neutral-900 border border-neutral-700 rounded-lg shadow-2xl w-full max-w-4xl h-[80vh] flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-neutral-800 bg-neutral-900 rounded-t-lg">
          <div className="flex items-center">
            <i className="fas fa-terminal text-blue-500 text-xl mr-3" />
            <h2 className="text-lg font-bold text-white">Server Logs</h2>
          </div>
          <button
            onClick={onClose}
            className="text-gray-400 hover:text-white p-1"
          >
            <i className="fas fa-times text-lg" />
          </button>
        </div>

        {/* Controls */}
        <div className="p-3 bg-neutral-800 border-b border-gray-700 flex items-center">
          <div className="relative flex-1">
            <i className="fas fa-search absolute left-3 top-1/2 transform -translate-y-1/2 text-gray-500" />
            <input
              type="text"
              placeholder="Filter logs..."
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              className="w-full pl-10 pr-3 py-2 bg-neutral-900 text-gray-200 rounded border border-gray-700 focus:outline-none focus:ring-1 focus:ring-blue-500"
            />
          </div>
          <div className="flex ml-3 space-x-2">
            <button
              onClick={() => setAutoScroll(!autoScroll)}
              className={`p-2 rounded ${autoScroll ? "bg-blue-600" : "bg-neutral-700"} hover:bg-blue-700`}
              title="Auto-scroll"
            >
              <i className={`fas ${autoScroll ? "fa-lock" : "fa-lock-open"} text-white text-sm`} />
            </button>
            <button
              onClick={() => setLogs([])}
              className="p-2 rounded bg-neutral-700 hover:bg-neutral-600"
              title="Clear logs"
            >
              <i className="fas fa-trash text-white text-sm" />
            </button>
            <button
              onClick={() => navigator.clipboard.writeText(logs.join("\n"))}
              className="p-2 rounded bg-neutral-700 hover:bg-neutral-600"
              title="Copy logs"
            >
              <i className="fas fa-copy text-white text-sm" />
            </button>
          </div>
        </div>

        {/* Log content */}
        <div className="flex-1 overflow-y-auto bg-neutral-950 p-3 font-mono text-sm">
          {filteredLogs.length === 0 ? (
            <div className="text-gray-500 italic p-4 text-center">
              {filter ? "No matching logs" : "No logs available"}
            </div>
          ) : (
            <div className="space-y-1">
              {filteredLogs.map((log, index) => (
                <div
                  key={index}
                  className="flex items-start hover:bg-neutral-900 px-2 py-1 rounded"
                >
                  {getLogIcon(log)}
                  <span className={`flex-1 ${
                    log.includes("Error") || log.includes("error")
                      ? "text-red-400"
                      : log.includes("Warning") || log.includes("warning")
                      ? "text-yellow-400"
                      : "text-gray-300"
                  }`}>
                    {log}
                  </span>
                </div>
              ))}
              <div ref={logsEndRef} />
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="p-2 text-xs text-gray-500 border-t border-gray-800 bg-neutral-900 rounded-b-lg flex justify-between">
          <span>
            <i className="fas fa-filter mr-1" />
            {filteredLogs.length} of {logs.length} shown
          </span>
          <span>
            <i className="fas fa-sync-alt mr-1" />
            Auto-refresh: 2s
          </span>
        </div>
      </div>
    </div>
  );
}

export default LogViewer;
