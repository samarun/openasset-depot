import { CheckCircle2, MessageCircle, Plus } from "lucide-react";
import { EmptyState } from "../components/EmptyState";
import type { Review } from "../types/domain";

interface ReviewsProps {
  reviews: Review[];
  onStartReview: () => void;
}

export function Reviews({ reviews, onStartReview }: ReviewsProps) {
  return (
    <main className="page">
      <header className="page-heading">
        <div>
          <span className="eyebrow">Reviews</span>
          <h1>Creative Reviews</h1>
        </div>
        <button className="primary-button" type="button" onClick={onStartReview}>
          <Plus size={17} />
          Create Review
        </button>
      </header>
      {reviews.length > 0 ? (
        <section className="card-list">
          {reviews.map((review) => (
            <article className="review-card" key={review.id}>
              <header className="card-header-row">
                <div>
                  <h3>{review.title}</h3>
                  <p>{review.owner}</p>
                </div>
              </header>
              <div className="review-status-row">
                {review.status === "Approved" ? <CheckCircle2 size={18} /> : <MessageCircle size={18} />}
                <strong>{review.status}</strong>
                <span>{review.files} files</span>
              </div>
            </article>
          ))}
        </section>
      ) : (
        <EmptyState
          icon={MessageCircle}
          title="No reviews in progress"
          detail="Choose submitted work to begin a focused creative review."
          action={<button className="primary-button" type="button" onClick={onStartReview}>Review Changes</button>}
        />
      )}
    </main>
  );
}
