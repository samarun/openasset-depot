import type { LucideIcon } from "lucide-react";

interface EmptyStateProps {
  icon: LucideIcon;
  title: string;
  detail: string;
  action?: React.ReactNode;
}

export function EmptyState({ icon: Icon, title, detail, action }: EmptyStateProps) {
  return (
    <section className="empty-state">
      <Icon size={30} />
      <h2>{title}</h2>
      <p>{detail}</p>
      {action}
    </section>
  );
}
