import { useEffect, useRef } from "react";
import { PortInfo, SortConfig, SortField } from "../types/port";

interface PortListProps {
  ports: PortInfo[];
  matchedPortIds: Set<string>;
  selected: Record<string, boolean>;
  onToggleProcess: (pid: number) => void;
  sortConfig: SortConfig;
  onSort: (field: SortField) => void;
}

interface ProcessGroup {
  pid: number;
  processName: string;
  processPath: string;
  commandLine: string;
  ports: PortInfo[];
}

interface SortableHeaderProps {
  label: string;
  field: SortField;
  sortConfig: SortConfig;
  onSort: (field: SortField) => void;
}

interface ProcessCheckboxProps {
  checked: boolean;
  indeterminate: boolean;
  onChange: () => void;
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

function ProcessCheckbox({ checked, indeterminate, onChange }: ProcessCheckboxProps) {
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (inputRef.current) {
      inputRef.current.indeterminate = indeterminate;
    }
  }, [indeterminate]);

  return (
    <input
      ref={inputRef}
      type="checkbox"
      checked={checked}
      aria-checked={indeterminate ? "mixed" : checked}
      onClick={(e) => e.stopPropagation()}
      onChange={onChange}
    />
  );
}

function buildProcessGroups(ports: PortInfo[]): ProcessGroup[] {
  const groups = new Map<number, ProcessGroup>();

  for (const port of ports) {
    const group = groups.get(port.pid);
    if (group) {
      group.ports.push(port);
    } else {
      groups.set(port.pid, {
        pid: port.pid,
        processName: port.processName,
        processPath: port.processPath,
        commandLine: port.commandLine,
        ports: [port],
      });
    }
  }

  return Array.from(groups.values());
}

function groupSortValue(group: ProcessGroup, field: SortField): number | string {
  switch (field) {
    case "port":
      return Math.min(...group.ports.map((port) => port.port));
    case "protocol":
      return group.ports.map((port) => port.protocol).join(",");
    case "pid":
      return group.pid;
    case "processName":
      return group.processName;
    case "processPath":
      return group.processPath;
    case "commandLine":
      return group.commandLine;
    case "state":
      return group.ports.map((port) => port.state).join(",");
    case "localAddress":
      return group.ports.map((port) => port.localAddress).join(",");
  }
}

function sortGroups(groups: ProcessGroup[], sortConfig: SortConfig): ProcessGroup[] {
  return [...groups].sort((a, b) => {
    const aVal = groupSortValue(a, sortConfig.field);
    const bVal = groupSortValue(b, sortConfig.field);
    let cmp = 0;

    if (typeof aVal === "number" && typeof bVal === "number") {
      cmp = aVal - bVal;
    } else {
      cmp = String(aVal).localeCompare(String(bVal), "zh-CN");
    }

    return sortConfig.order === "asc" ? cmp : -cmp;
  });
}

function portLabel(port: PortInfo): string {
  return `${port.protocol} ${port.localAddress}`;
}

export function PortList({
  ports,
  matchedPortIds,
  selected,
  onToggleProcess,
  sortConfig,
  onSort,
}: PortListProps) {
  if (ports.length === 0) {
    return <div className="empty-state">暂无端口占用数据</div>;
  }

  const groups = sortGroups(buildProcessGroups(ports), sortConfig);

  return (
    <div className="port-list">
      <table>
        <thead>
          <tr>
            <th>选择</th>
            <SortableHeader label="进程名" field="processName" sortConfig={sortConfig} onSort={onSort} />
            <SortableHeader label="PID" field="pid" sortConfig={sortConfig} onSort={onSort} />
            <SortableHeader label="端口" field="port" sortConfig={sortConfig} onSort={onSort} />
            <SortableHeader label="路径" field="processPath" sortConfig={sortConfig} onSort={onSort} />
            <SortableHeader label="命令行" field="commandLine" sortConfig={sortConfig} onSort={onSort} />
          </tr>
        </thead>
        <tbody>
          {groups.map((group) => {
            const selectedPorts = group.ports.filter((port) => selected[port.id]);
            const matchedCount = group.ports.filter((port) => matchedPortIds.has(port.id)).length;
            const isSelected = selectedPorts.length === group.ports.length;
            const isPartial = selectedPorts.length > 0 && !isSelected;
            const metaText =
              matchedCount > 0 && matchedCount < group.ports.length
                ? `${matchedCount}/${group.ports.length} 条匹配记录`
                : `${group.ports.length} 条端口记录`;

            return (
              <tr
                key={group.pid}
                className={isSelected || isPartial ? "selected-row" : ""}
                onClick={() => onToggleProcess(group.pid)}
              >
                <td>
                  <ProcessCheckbox
                    checked={isSelected}
                    indeterminate={isPartial}
                    onChange={() => onToggleProcess(group.pid)}
                  />
                </td>
                <td className="process-cell">
                  <div className="process-name">{group.processName}</div>
                  <div className="process-meta">{metaText}</div>
                </td>
                <td>{group.pid}</td>
                <td>
                  <div className="port-tags">
                    {group.ports.map((port) => (
                      <span
                        className={`port-tag ${matchedPortIds.has(port.id) ? "matched" : ""}`}
                        key={port.id}
                        title={`${port.state} ${portLabel(port)}`}
                      >
                        {port.port}
                        <span className="port-protocol">{port.protocol}</span>
                      </span>
                    ))}
                  </div>
                </td>
                <td className="path-cell" title={group.processPath}>
                  {group.processPath || "-"}
                </td>
                <td className="command-cell" title={group.commandLine}>
                  {group.commandLine || "-"}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
