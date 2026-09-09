// Tracks the newest queued layout, not just the last acknowledgment. A→B→A
// must send the second A even while B is still waiting on the native command lane.
export class NativeLayoutCommands<T> {
  private sequence = 0
  private pending: { sequence: number; value: T } | null = null
  private applied: { sequence: number; value: T } | null = null
  private readonly equals: (left: T | null, right: T) => boolean

  constructor(equals: (left: T | null, right: T) => boolean) {
    this.equals = equals
  }

  enqueue(value: T): number | null {
    if (this.equals(this.pending?.value ?? this.applied?.value ?? null, value)) return null
    this.sequence += 1
    this.pending = { sequence: this.sequence, value }
    return this.sequence
  }

  settle(sequence: number, value: T, accepted: boolean): boolean {
    if (accepted && sequence > (this.applied?.sequence ?? 0)) {
      this.applied = { sequence, value }
    }
    const latest = this.pending?.sequence === sequence
    if (latest) this.pending = null
    return latest
  }
}
