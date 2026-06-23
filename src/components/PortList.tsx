import { PortInfo, SortConfig, SortField } from "../types/port";

interface PortListProps {
  ports: PortInfo[];
  selected: Record<string, boolean>;
  onToggleRow: (id: string) => void;
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

export function PortList({ ports, selected, onToggleRow, sortConfig, onSort }: PortListProps) {
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
            <SortableHeader label="路径" field="processPath" sortConfig={sortConfig} onSort={onSort} />
            <SortableHeader label="命令行" field="commandLine" sortConfig={sortConfig} onSort={onSort} />
            <SortableHeader label="状态" field="state" sortConfig={sortConfig} onSort={onSort} />
            <SortableHeader label="本地地址" field="localAddress" sortConfig={sortConfig} onSort={onSort} />
          </tr>
        </thead>
        <tbody>
          {ports.map((port) => (
            <tr
              key={port.id}
              className={selected[port.id] ? "selected-row" : ""}
              onClick={() => onToggleRow(port.id)}
            >
              <td>
                <input
                  type="checkbox"
                  checked={!!selected[port.id]}
                  onClick={(e) => e.stopPropagation()}
                  onChange={() => onToggleRow(port.id)}
                />
              </td>
              <td>{port.port}</td>
              <td>{port.protocol}</td>
              <td>{port.pid}</td>
              <td>{port.processName}</td>
              <td className="path-cell" title={port.processPath}>
                {port.processPath || "-"}
              </td>
              <td className="command-cell" title={port.commandLine}>
                {port.commandLine || "-"}
              </td>
              <td>{port.state}</td>
              <td>{port.localAddress}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
