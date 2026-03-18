interface SearchBarProps {
  value: string;
  onChange: (value: string) => void;
}

export function SearchBar({ value, onChange }: SearchBarProps) {
  return (
    <div className="search-bar">
      <input
        type="text"
        placeholder="搜索端口号..."
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  );
}