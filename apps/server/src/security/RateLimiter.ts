export class TokenBucket {
  private tokens: number;
  private lastRefill: number;

  constructor(
    private readonly capacity: number,
    private readonly refillPerSecond: number,
  ) {
    this.tokens = capacity;
    this.lastRefill = Date.now();
  }

  public tryConsume(count = 1): boolean {
    const now = Date.now();
    const elapsedSeconds = (now - this.lastRefill) / 1000;
    this.tokens = Math.min(this.capacity, this.tokens + elapsedSeconds * this.refillPerSecond);
    this.lastRefill = now;

    if (this.tokens >= count) {
      this.tokens -= count;
      return true;
    }
    return false;
  }
}

export class SocketRateLimiter {
  // Movement changes: burst 25, 20/s
  private moveLimiter = new TokenBucket(25, 20);
  // Combat actions: burst 12, 8/s
  private actionLimiter = new TokenBucket(12, 8);
  // Purchases: burst 8, 5/s
  private buyLimiter = new TokenBucket(8, 5);

  public checkMove(): boolean {
    return this.moveLimiter.tryConsume(1);
  }

  public checkAction(): boolean {
    return this.actionLimiter.tryConsume(1);
  }

  public checkBuy(): boolean {
    return this.buyLimiter.tryConsume(1);
  }
}
