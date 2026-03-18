export interface PortInfo {
  port: number;
  protocol: string;
  pid: number;
  processName: string;
  state: string;
  localAddress: string;
}

export interface KillResult {
  pid: number;
  success: boolean;
  message: string;
}

export type SortField = "port" | "protocol" | "pid" | "processName" | "state" | "localAddress";
export type SortOrder = "asc" | "desc";

export interface SortConfig {
  field: SortField;
  order: SortOrder;
}