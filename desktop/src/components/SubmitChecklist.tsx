import { AlertTriangle, CheckCircle2, Circle } from "lucide-react";
import type { ValidationMessage } from "../types/domain";

interface SubmitChecklistProps {
  warnings: ValidationMessage[];
  errors: ValidationMessage[];
}

const checklistItems = [
  "Required files locked",
  "No missing dependencies",
  "Unity .meta files included",
  "Large files acknowledged",
  "Generated cache excluded",
  "Path validation clear",
];

export function SubmitChecklist({ warnings, errors }: SubmitChecklistProps) {
  const hasIssues = warnings.length > 0 || errors.length > 0;
  return (
    <div className="submit-checklist">
      {checklistItems.map((item, index) => {
        const warning = hasIssues && index === 1;
        const Icon = warning ? AlertTriangle : index < 2 ? CheckCircle2 : Circle;
        return (
          <span key={item} className={`checklist-row ${warning ? "is-warning" : ""}`}>
            <Icon size={16} />
            {item}
          </span>
        );
      })}
      {warnings.map((warning) => (
        <span key={`${warning.code}-${warning.path}`} className="checklist-warning">
          <AlertTriangle size={16} />
          {warning.message}
        </span>
      ))}
      {errors.map((error) => (
        <span key={`${error.code}-${error.path}`} className="checklist-error">
          <AlertTriangle size={16} />
          {error.message}
        </span>
      ))}
    </div>
  );
}
