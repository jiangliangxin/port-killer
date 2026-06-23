export interface PortInfo {
  id: string;
  port: number;
  protocol: string;
  pid: number;
  processName: string;
  processPath: string;
  commandLine: string;
  state: string;
  localAddress: string;
}

export type RawPortInfo = Omit<PortInfo, "id">;

export interface KillTarget {
  port: number;
  protocol: string;
  pid: number;
  localAddress: string;
}

export interface KillResult {
  pid: number;
  success: boolean;
  status:
    | "released"
    | "notReleased"
    | "skipped"
    | "failed"
    | "closed"
    | "permissionDenied"
    | "reoccupied";
  message: string;
}

export interface AppStatus {
  elevated: boolean;
}

export type SortField =
  | "port"
  | "protocol"
  | "pid"
  | "processName"
  | "processPath"
  | "commandLine"
  | "state"
  | "localAddress";
export type SortOrder = "asc" | "desc";

export interface SortConfig {
  field: SortField;
  order: SortOrder;
}
