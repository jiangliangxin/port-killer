import { useState, useMemo, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { usePorts } from "./hooks/usePorts";
import { SearchBar } from "./components/SearchBar";
import { PortList } from "./components/PortList";
import { ActionBar } from "./components/ActionBar";
import { KillResult, SortConfig, SortField } from "./types/port";

const DEFAULT_SORT: SortConfig = { field: "port", order: "asc" };

function App() {
  const { ports, loading, error, scanPorts } = usePorts();
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedPids, setSelectedPids] = useState<Set<number>>(new Set());
  const [isKilling, setIsKilling] = useState(false);
  const [killResults, setKillResults] = useState<KillResult[]>([]);
  const [sortConfig, setSortConfig] = useState<SortConfig>(DEFAULT_SORT);
  const [showConfirm, setShowConfirm] = useState(false);
  const [autoRefresh, setAutoRefresh] = useState(false);

  // 自动刷新
  useEffect(() => {
    if (!autoRefresh) return;
    const interval = setInterval(() => {
      scanPorts();
    }, 5000);
    return () => clearInterval(interval);
  }, [autoRefresh, scanPorts]);

  const handleSort = useCallback((field: SortField) => {
    setSortConfig((prev) => ({
      field,
      order: prev.field === field && prev.order === "asc" ? "desc" : "asc",
    }));
  }, []);

  const filteredAndSortedPorts = useMemo(() => {
    let result = ports;

    if (searchQuery) {
      const query = searchQuery.toLowerCase();
      result = result.filter(
        (port) =>
          port.port.toString().includes(searchQuery) ||
          port.processName.toLowerCase().includes(query)
      );
    }

    result = [...result].sort((a, b) => {
      const { field, order } = sortConfig;
      let cmp = 0;

      const aVal = a[field];
      const bVal = b[field];

      if (typeof aVal === "number" && typeof bVal === "number") {
        cmp = aVal - bVal;
      } else {
        cmp = String(aVal).localeCompare(String(bVal), "zh-CN");
      }

      return order === "asc" ? cmp : -cmp;
    });

    return result;
  }, [ports, searchQuery, sortConfig]);

  const togglePid = (pid: number) => {
    setSelectedPids((prev) => {
      const next = new Set(prev);
      if (next.has(pid)) {
        next.delete(pid);
      } else {
        next.add(pid);
      }
      return next;
    });
  };

  const selectAll = useCallback(() => {
    const allPids = new Set(filteredAndSortedPorts.map((p) => p.pid));
    setSelectedPids(allPids);
  }, [filteredAndSortedPorts]);

  const clearSelection = useCallback(() => {
    setSelectedPids(new Set());
  }, []);

  const handleKill = () => {
    if (selectedPids.size === 0) return;
    setShowConfirm(true);
  };

  const confirmKill = async () => {
    setShowConfirm(false);
    setIsKilling(true);
    try {
      const results = await invoke<KillResult[]>("kill_processes", {
        pids: Array.from(selectedPids),
      });
      setKillResults(results);

      const successPids = results
        .filter((r) => r.success)
        .map((r) => r.pid);
      setSelectedPids((prev) => {
        const next = new Set(prev);
        successPids.forEach((pid) => next.delete(pid));
        return next;
      });

      await scanPorts();
    } catch (e) {
      console.error("Failed to kill processes:", e);
    } finally {
      setIsKilling(false);
    }
  };

  return (
    <div className="app">
      <header>
        <h1>端口占用管理工具</h1>
      </header>

      <main>
        <ActionBar
          selectedCount={selectedPids.size}
          totalCount={new Set(filteredAndSortedPorts.map(p => p.pid)).size}
          onRefresh={scanPorts}
          onKill={handleKill}
          onSelectAll={selectAll}
          onClearSelection={clearSelection}
          isKilling={isKilling}
          autoRefresh={autoRefresh}
          onToggleAutoRefresh={() => setAutoRefresh(!autoRefresh)}
        />

        <SearchBar value={searchQuery} onChange={setSearchQuery} />

        {loading && <div className="loading">正在扫描端口...</div>}
        {error && <div className="error">{error}</div>}

        <PortList
          ports={filteredAndSortedPorts}
          selectedPids={selectedPids}
          onTogglePid={togglePid}
          sortConfig={sortConfig}
          onSort={handleSort}
        />
      </main>

      {killResults.length > 0 && (
        <div className="kill-results">
          <h3>操作结果</h3>
          <ul>
            {killResults.map((result) => (
              <li
                key={result.pid}
                className={result.success ? "success" : "failed"}
              >
                PID {result.pid}: {result.message}
              </li>
            ))}
          </ul>
        </div>
      )}

      {showConfirm && (
        <div className="modal-overlay">
          <div className="modal">
            <h3>确认终止进程</h3>
            <p>确定要终止选中的 {selectedPids.size} 个进程吗？</p>
            <p className="warning">警告：强制终止可能导致数据丢失</p>
            <div className="modal-actions">
              <button className="btn-cancel" onClick={() => setShowConfirm(false)}>
                取消
              </button>
              <button className="btn-confirm" onClick={confirmKill}>
                确认终止
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;