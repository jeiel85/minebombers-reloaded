export class ClockSync {
  public rtt = 0;
  public serverOffset = 0;
  private pings = new Map<number, number>();
  private pingCounter = 0;

  public createPing(): { n: number; sentAt: number } {
    const n = ++this.pingCounter;
    const sentAt = Date.now();
    this.pings.set(n, sentAt);
    return { n, sentAt };
  }

  public handlePong(n: number, serverTime: number): void {
    const sentAt = this.pings.get(n);
    if (sentAt === undefined) return;
    this.pings.delete(n);

    const now = Date.now();
    const currentRtt = now - sentAt;
    // Exponential moving average for smooth RTT
    this.rtt = this.rtt === 0 ? currentRtt : Math.round(this.rtt * 0.7 + currentRtt * 0.3);
    const estimatedOffset = serverTime + (currentRtt / 2) - now;
    this.serverOffset = Math.round(this.serverOffset === 0 ? estimatedOffset : this.serverOffset * 0.7 + estimatedOffset * 0.3);
  }

  public getServerTime(): number {
    return Date.now() + this.serverOffset;
  }
}
