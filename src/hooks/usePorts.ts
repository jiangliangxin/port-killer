import { useState, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { PortInfo, RawPortInfo } from "../types/port";

function buildPortId(port: RawPortInfo): string {
  return `${port.protocol}|${port.localAddress}|${port.pid}|${port.state}`;
}

function normalizePorts(ports: RawPortInfo[]): PortInfo[] {
  const normalized = new Map<string, PortInfo>();

  for (const port of ports) {
    const id = buildPortId(port);

    // Windows UDP 表里可能出现完全重复的记录，前端只展示一个可操作目标。
    if (!normalized.has(id)) {
      normalized.set(id, { ...port, id });
    }
  }

  return Array.from(normalized.values());
}

export function usePorts() {
  const [ports, setPorts] = useState<PortInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const scanningRef = useRef(false);

  const scanPorts = useCallback(async () => {
    if (scanningRef.current) return;
    scanningRef.current = true;
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<RawPortInfo[]>("scan_ports");
      setPorts(normalizePorts(result));
    } catch (e) {
      setError(String(e));
    } finally {
      scanningRef.current = false;
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    scanPorts();
  }, [scanPorts]);

  return { ports, loading, error, scanPorts };
}
