import { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { PortInfo } from "../types/port";

export function usePorts() {
  const [ports, setPorts] = useState<PortInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const scanPorts = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<PortInfo[]>("scan_ports");
      setPorts(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    scanPorts();
  }, [scanPorts]);

  return { ports, loading, error, scanPorts };
}