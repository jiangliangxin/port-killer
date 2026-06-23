import { useState, useMemo, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { usePorts } from "./hooks/usePorts";
import { SearchBar } from "./components/SearchBar";
import { PortList } from "./components/PortList";
import { ActionBar } from "./components/ActionBar";
import { KillResult, KillTarget, SortConfig, SortField } from "./types/port";

const DEFAULT_SORT: SortConfig = { field: "port", order: "asc" };

function App() {
  const { ports, loading, error, scanPorts } = usePorts();
  const [searchQuery, setSearchQuery] = useState("");
  const [selected, setSelected] = useState<Record<string, boolean>>({});
  const [isKilling, setIsKilling] = useState(false);
  const [killResults, setKillResults] = useState<KillResult[]>([]);
  const [sortConfig, setSortConfig] = useState<SortConfig>(DEFAULT_SORT);
  const [showConfirm, setShowConfirm] = useState(false);
  const [autoRefresh, setAutoRefresh] = useState(false);

  // 自动刷新端口列表，适合排查 dev server 反复占用端口的场景。
  useEffect(() => {
    if (!autoRefresh) return;
    const interval = setInterval(() => {
      scanPorts();
    }, 5000);
    return () => clearInterval(interval);
  }, [autoRefresh, scanPorts]);

  useEffect(() => {
    const validIds = new Set(ports.map((port) => port.id));
    setSelected((prev) => {
      const next: Record<string, boolean> = {};
      let changed = false;

      for (const id of Object.keys(prev)) {
        if (validIds.has(id)) {
          next[id] = true;
        } else {
          changed = true;
        }
      }

      return changed ? next : prev;
    });
  }, [ports]);

  const handleSort = useCallback((field: SortField) => {
    setSortConfig((prev) => ({
      field,
      order: prev.field === field && prev.order === "asc" ? "desc" : "asc",
    }));
  }, []);

  const filteredAndSortedPorts = useMemo(() => {
    let result = ports;
    const query = searchQuery.trim().toLowerCase();

    if (query) {
      result = result.filter(
        (port) =>
          port.port.toString().includes(query) ||
          port.pid.toString().includes(query) ||
          port.protocol.toLowerCase().includes(query) ||
          port.processName.toLowerCase().includes(query) ||
          port.processPath.toLowerCase().includes(query) ||
          port.commandLine.toLowerCase().includes(query) ||
          port.state.toLowerCase().includes(query) ||
          port.localAddress.toLowerCase().includes(query)
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

  const selectedPorts = useMemo(
    () => ports.filter((port) => selected[port.id]),
    [ports, selected]
  );

  const selectedPortCount = selectedPorts.length;
  const selectedProcessCount = useMemo(
    () => new Set(selectedPorts.map((port) => port.pid)).size,
    [selectedPorts]
  );

  const toggleRow = useCallback((id: string) => {
    setSelected((prev) => {
      const next = { ...prev };
      if (next[id]) {
        delete next[id];
      } else {
        next[id] = true;
      }
      return next;
    });
  }, []);

  const selectAll = useCallback(() => {
    const next: Record<string, boolean> = {};
    for (const p of filteredAndSortedPorts) {
      next[p.id] = true;
    }
    setSelected(next);
  }, [filteredAndSortedPorts]);

  const clearSelection = useCallback(() => {
    setSelected({});
  }, []);

  const handleKill = useCallback(() => {
    if (selectedPortCount === 0) return;
    setShowConfirm(true);
  }, [selectedPortCount]);

  const confirmKill = async (force: boolean) => {
    setShowConfirm(false);
    setIsKilling(true);
    try {
      const targets: KillTarget[] = selectedPorts.map((port) => ({
        port: port.port,
        protocol: port.protocol,
        pid: port.pid,
        localAddress: port.localAddress,
      }));
      const results = await invoke<KillResult[]>("kill_processes", {
        targets,
        force,
      });
      setKillResults(results);

      const releasedPids = new Set(
        results
          .filter((result) => result.success || result.status === "released")
          .map((result) => result.pid)
      );
      setSelected((prev) => {
        const next = { ...prev };
        for (const port of selectedPorts) {
          if (releasedPids.has(port.pid)) {
            delete next[port.id];
          }
        }
        return next;
      });
      await scanPorts();
    } catch (e) {
      console.error("Failed to kill processes:", e);
      setKillResults([
        {
          pid: 0,
          success: false,
          status: "failed",
          message: `关闭失败: ${String(e)}`,
        },
      ]);
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
          selectedCount={selectedProcessCount}
          totalCount={filteredAndSortedPorts.length}
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
          selected={selected}
          onToggleRow={toggleRow}
          sortConfig={sortConfig}
          onSort={handleSort}
        />
      </main>

      {killResults.length > 0 && (
        <div className="kill-results">
          <h3>操作结果</h3>
          <ul>
            {killResults.map((result, index) => (
              <li
                key={`${result.pid}-${result.status}-${index}`}
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
            <h3>确认关闭进程</h3>
            <p>
              确定要关闭 {selectedProcessCount} 个进程吗？涉及 {selectedPortCount} 条端口记录。
            </p>
            <p className="warning">建议先正常关闭；端口未释放时再强制关闭。</p>
            <div className="modal-actions">
              <button className="btn-cancel" onClick={() => setShowConfirm(false)}>
                取消
              </button>
              <button className="btn-force" onClick={() => confirmKill(true)}>
                强制关闭
              </button>
              <button className="btn-confirm" onClick={() => confirmKill(false)}>
                正常关闭
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
