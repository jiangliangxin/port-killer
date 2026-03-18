import { PortInfo, SortConfig, SortField } from "../types/port";

interface PortListProps {
  ports: PortInfo[];
  selectedPids: Set<number>;
  onTogglePid: (pid: number) => void;
  sortConfig: SortConfig;
  onSort: (field: SortField) => void;
}

interface SortableHeaderProps {
  label: string;
  field: SortField;
  sortConfig: SortConfig;
  onSort: (field: SortField) => void;
}

function SortableHeader({ label, field, sortConfig, onSort }: SortableHeaderProps) {
  const isActive = sortConfig.field === field;
  const arrow = isActive ? (sortConfig.order === "asc" ? " ▲" : " ▼") : "";

  return (
    <th
      className={`sortable ${isActive ? "active" : ""}`}
      onClick={() => onSort(field)}
    >
      {label}{arrow}
    </th>
  );
}

export function PortList({ ports, selectedPids, onTogglePid, sortConfig, onSort }: PortListProps) {
  if (ports.length === 0) {
    return <div className="empty-state">暂无端口占用数据</div>;
  }

  return (
    <div className="port-list">
      <table>
        <thead>
          <tr>
            <th>选择</th>
            <SortableHeader label="端口" field="port" sortConfig={sortConfig} onSort={onSort} />
            <SortableHeader label="协议" field="protocol" sortConfig={sortConfig} onSort={onSort} />
            <SortableHeader label="PID" field="pid" sortConfig={sortConfig} onSort={onSort} />
            <SortableHeader label="进程名" field="processName" sortConfig={sortConfig} onSort={onSort} />
            <SortableHeader label="状态" field="state" sortConfig={sortConfig} onSort={onSort} />
            <SortableHeader label="本地地址" field="localAddress" sortConfig={sortConfig} onSort={onSort} />
          </tr>
        </thead>
        <tbody>
          {ports.map((port, index) => (
            <tr key={`${port.pid}-${port.port}-${index}`}>
              <td>
                <input
                  type="checkbox"
                  checked={selectedPids.has(port.pid)}
                  onChange={() => onTogglePid(port.pid)}
                />
              </td>
              <td>{port.port}</td>
              <td>{port.protocol}</td>
              <td>{port.pid}</td>
              <td>{port.processName}</td>
              <td>{port.state}</td>
              <td>{port.localAddress}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}