interface ActionBarProps {
  selectedCount: number;
  totalCount: number;
  onRefresh: () => void;
  onKill: () => void;
  onSelectAll: () => void;
  onClearSelection: () => void;
  isKilling: boolean;
  autoRefresh: boolean;
  onToggleAutoRefresh: () => void;
}

export function ActionBar({
  selectedCount,
  totalCount,
  onRefresh,
  onKill,
  onSelectAll,
  onClearSelection,
  isKilling,
  autoRefresh,
  onToggleAutoRefresh,
}: ActionBarProps) {
  return (
    <div className="action-bar">
      <button onClick={onRefresh} className="btn-refresh">
        刷新列表
      </button>
      <button
        onClick={onToggleAutoRefresh}
        className={autoRefresh ? "btn-auto-active" : "btn-auto"}
      >
        {autoRefresh ? "自动刷新中" : "自动刷新"}
      </button>
      <button onClick={onSelectAll} className="btn-select" disabled={totalCount === 0}>
        全选
      </button>
      <button onClick={onClearSelection} className="btn-select" disabled={selectedCount === 0}>
        取消选择
      </button>
      <button
        onClick={onKill}
        disabled={selectedCount === 0 || isKilling}
        className="btn-kill"
      >
        {isKilling
          ? "正在关闭..."
          : `关闭选中进程 (${selectedCount})`}
      </button>
    </div>
  );
}