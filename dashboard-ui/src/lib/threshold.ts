/** The single threshold scale (ISC-16): every health-like number in the UI
 * maps to a color/label through these two functions and nowhere else. */

export type ThresholdBand = 'healthy' | 'moderate' | 'elevated' | 'critical';

/** Map a 0–100 score to its band. Bands: ≥80 healthy · ≥60 moderate · ≥40 elevated · <40 critical. */
export function thresholdBand(score: number): ThresholdBand {
  if (score >= 80) return 'healthy';
  if (score >= 60) return 'moderate';
  if (score >= 40) return 'elevated';
  return 'critical';
}

export function thresholdColor(score: number): string {
  return `var(--threshold-${thresholdBand(score)})`;
}

export const bandLabels: Record<ThresholdBand, string> = {
  healthy: 'Healthy',
  moderate: 'Moderate',
  elevated: 'At risk',
  critical: 'Critical',
};
