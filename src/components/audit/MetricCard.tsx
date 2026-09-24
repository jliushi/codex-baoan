import type { LucideIcon } from "lucide-react";

interface MetricCardProps {
  label: string;
  value: number;
  caption: string;
  icon: LucideIcon;
  tone?: "blue" | "red" | "green" | "amber";
}

const numberFormatter = new Intl.NumberFormat("zh-CN");

export function MetricCard({
  label,
  value,
  caption,
  icon: Icon,
  tone = "blue",
}: MetricCardProps) {
  return (
    <article className={`metric-card tone-${tone}`}>
      <div className="metric-topline">
        <span className="metric-label">{label}</span>
        <span className="metric-icon">
          <Icon size={17} strokeWidth={1.9} aria-hidden="true" />
        </span>
      </div>
      <strong className="metric-value">{numberFormatter.format(value)}</strong>
      <span className="metric-caption">{caption}</span>
    </article>
  );
}
