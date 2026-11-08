Poll stability (10 runs, jitter_ms=40, seed=42)
pass_rate: 10/10
poll_attempts: min=4 max=6 unique=3
jitter_source_hypothesis: likely mock-sut payment delay jitter (seeded); review volatile poll ts spread

| Run | Exit | PollAttempts | WaitDurationMs |
|---:|---:|---:|---:|
| 1 | 0 | 5 |  |
| 2 | 0 | 4 | 1 |
| 3 | 0 | 5 |  |
| 4 | 0 | 5 | 1 |
| 5 | 0 | 5 | 1 |
| 6 | 0 | 5 | 1 |
| 7 | 0 | 5 | 1 |
| 8 | 0 | 6 |  |
| 9 | 0 | 5 | 1 |
| 10 | 0 | 5 | 1 |
