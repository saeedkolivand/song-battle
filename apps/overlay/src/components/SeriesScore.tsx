function Pips({ filled, total, fillClass }: { filled: number; total: number; fillClass: string }) {
  return (
    <span className="inline-flex gap-[0.4vw]" aria-hidden="true">
      {Array.from({ length: total }, (_, i) => (
        <span key={i} className={`h-[1vw] w-[1vw] rounded-full ${i < filled ? fillClass : 'bg-white/20'}`} />
      ))}
    </span>
  );
}

// Best-of series score for the current match: win pips per side + the wins tally.
// Null for single games (bestOf 1) so the Stage can render it unconditionally.
export function SeriesScore({ winsA, winsB, bestOf }: { winsA: number; winsB: number; bestOf: number }) {
  if (bestOf <= 1) return null;
  const need = Math.floor(bestOf / 2) + 1;
  return (
    <div
      className="flex items-center gap-[1vw] rounded-full border border-white/10 bg-black/30 px-[1vw] py-[0.4vw] backdrop-blur-md"
      role="img"
      aria-label={`Series score ${winsA} to ${winsB}`}
    >
      <Pips filled={winsA} total={need} fillClass="bg-a" />
      <span className="sb-shadow text-[1.6vw] font-black tabular-nums text-white">
        {winsA}–{winsB}
      </span>
      <Pips filled={winsB} total={need} fillClass="bg-b" />
    </div>
  );
}
