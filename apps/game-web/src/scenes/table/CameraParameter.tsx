/**
 * 镜头参数的一根滑杆。
 *
 * 牌桌设置页上的每一项都长这样：左边参数名、右边当前读数、下面一根滑杆。样式和
 * 取位写在这一处，几根滑杆才不会各读各的位数。
 */
export function CameraParameter({
  label,
  value,
  min,
  max,
  step,
  suffix = "",
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  suffix?: string;
  onChange: (value: number) => void;
}) {
  return (
    <label className="match-preview__parameter">
      <span>
        <b>{label}</b>
        <output>
          {round(value)}
          {suffix}
        </output>
      </span>
      <input
        type="range"
        value={value}
        min={min}
        max={max}
        step={step}
        onChange={(event) => onChange(Number(event.currentTarget.value))}
      />
    </label>
  );
}

/** 参数一律读到小数点后两位，多余的位数对调镜头没有意义。 */
export function round(value: number): number {
  return Math.round(value * 100) / 100;
}
