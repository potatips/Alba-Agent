interface MathFormulaProps {
  formula: "gor_update" | "gor_phi";
}

export function MathFormula({ formula }: MathFormulaProps) {
  if (formula === "gor_update") {
    return (
      <div className="my-3 flex items-center justify-center overflow-x-auto py-2 font-serif text-base tracking-wide text-zinc-100 select-all sm:text-lg">
        <span className="italic">x</span>
        <sub className="text-xs not-italic">t+1</sub>
        <span className="mx-2">=</span>
        <span className="italic">x</span>
        <sub className="text-xs not-italic">t</sub>
        <span className="mx-2">+</span>
        <span className="italic">u</span>
        <sub className="text-xs">
          <span className="italic">t</span>
          <span className="font-sans font-bold">⊥</span>
        </sub>
        <span className="mx-2">+</span>
        <span className="italic">ϕ</span>
        <span className="font-sans">(</span>
        <span className="italic">ρ</span>
        <sub className="text-xs italic">t</sub>
        <span className="font-sans">)</span>
        <span className="mx-1.5 font-sans">·</span>
        <span className="italic">u</span>
        <sub className="text-xs">
          <span className="italic">t</span>
          <span className="font-sans font-bold">∥</span>
        </sub>
      </div>
    );
  }

  return (
    <div className="my-3 flex flex-wrap items-center justify-center gap-2 overflow-x-auto py-2 font-serif text-base tracking-wide text-zinc-100 select-all sm:text-lg">
      <div className="flex items-center">
        <span className="italic">ϕ</span>
        <span className="font-sans">(</span>
        <span className="italic">ρ</span>
        <sub className="text-xs italic">t</sub>
        <span className="font-sans">)</span>
        <span className="mx-2">=</span>
        <span className="font-sans">2.0</span>
        <span className="mx-1.5 font-sans">·</span>
        <span className="italic">σ</span>
        <span className="font-sans">(</span>
        <span className="italic">k</span>
        <span className="mx-1 font-sans">·</span>
        <span className="italic">ρ</span>
        <sub className="text-xs italic">t</sub>
        <span className="font-sans">)</span>
        <span className="mx-2">=</span>
      </div>

      <div className="inline-flex flex-col items-center justify-center px-1">
        <span className="border-b border-zinc-400 px-3 pb-0.5 text-center font-sans text-sm font-medium">
          2
        </span>
        <span className="pt-0.5 text-center text-sm">
          <span className="font-sans">1</span>
          <span className="mx-1 font-sans">+</span>
          <span className="italic">e</span>
          <sup className="text-[10px]">
            <span className="font-sans">-</span>
            <span className="italic">k</span>
            <span className="font-sans">·</span>
            <span className="italic">ρ</span>
            <sub className="text-[8px] italic">t</sub>
          </sup>
        </span>
      </div>

      <div className="ml-3 font-sans text-xs text-zinc-400">
        (với <span className="font-serif italic text-zinc-200">k</span> = 3.0)
      </div>
    </div>
  );
}
