# Code Golf Platform

A modern code golf competition platform built with Rust, Axum, and SQLite. Host code golf challenges, track submissions by byte count, manage tournaments, and compete on global and per-problem leaderboards.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Features

- **Code Challenges**: Create and manage code golf problems with multiple test cases
- **Multi-Language Support**: Run submissions in Python, Bash, Ruby, Perl, Node.js, Lua, and more
- **Tournament System**: Create time-limited tournaments with active problem selection
- **Scoring System**: Byte-count based scoring with PAR (Par) score calculation
- **Leaderboards**: Global and per-problem scoreboards
- **Admin Dashboard**: Full admin panel for problem, user, and submission management
- **HTMX Integration**: Responsive, real-time UI with progressive enhancement
- **Secure**: Argon2 password hashing, secure session cookies, input validation

## Quick Start

### Prerequisites

- Docker 20.10+
- Docker Compose 2.0+
- Rust 1.75+ (for local development)

### Development with Docker

```bash
# Start the development server
docker-compose up --build

# The application will be available at http://localhost:3000
```

### Local Development

```bash
# Clone the repository
git clone https://github.com/yourusername/code-golf.git
cd code-golf

# Set up environment
cp .env.example .env
# Edit .env with your configuration

# Run migrations and seed data
cargo run -- migrate
cargo run -- seed

# Start the development server
cargo run
```

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URL` | SQLite connection string | `sqlite://code-golf.db` |
| `HOST` | Server bind address | `0.0.0.0` |
| `PORT` | Server port | `3000` |
| `RUST_LOG` | Logging level | `info` |
| `MAX_CODE_SIZE` | Maximum code size in bytes | `65536` |
| `TIME_LIMIT_MS` | Default time limit in ms | `5000` |
| `MEMORY_LIMIT_KB` | Default memory limit in KB | `65536` |
| `SESSION_EXPIRY_DAYS` | Session cookie expiry | `7` |
| `DATABASE_MAX_CONNECTIONS` | Max DB connections | `20` |
| `DATABASE_MIN_CONNECTIONS` | Min DB connections | `5` |

### Docker Compose Configuration

Development environment (`docker-compose.yml`):
- Hot reload for source files
- Development database in mounted volume
- Full debug logging

Production environment (`docker-compose.prod.yml`):
- Persistent data volumes
- Health checks
- Automatic restarts
- Production logging level

## Project Structure

```
code-golf/
├── src/
│   ├── admin/          # Admin dashboard handlers
│   ├── auth/           # Authentication handlers
│   ├── db/             # Database models and operations
│   ├── problems/       # Problem list and detail handlers
│   ├── profile/        # User profile handlers
│   ├── runner/         # Code execution sandbox
│   ├── scoreboard/     # Leaderboard handlers
│   ├── scoring/        # Scoring algorithms
│   ├── submissions/    # Submission handling and judging
│   ├── tournaments/    # Tournament management
│   ├── app.rs          # Application router and state
│   ├── config.rs       # Configuration management
│   ├── error.rs        # Error types
│   └── main.rs         # Application entry point
├── migrations/         # Database migrations
├── templates/          # MiniJinja templates
├── static/             # CSS, JS, and assets
├── tests/              # Integration tests
├── Cargo.toml          # Rust dependencies
├── docker-compose.yml  # Docker development setup
└── DEPLOYMENT.md       # Deployment guide
```

## API Endpoints

### Public Endpoints

- `GET /` - Home page with featured problems
- `GET /problems` - List all problems with filters
- `GET /problems/{slug}` - View problem details
- `POST /problems/{slug}/submit` - Submit code solution
- `GET /scoreboard` - Global leaderboard
- `GET /scoreboard/problem/{slug}` - Per-problem leaderboard
- `GET /tournaments` - List tournaments
- `GET /login` / `POST /login` - User login
- `GET /register` / `POST /register` - User registration
- `POST /logout` - User logout

### Admin Endpoints (Admin Only)

- `GET /admin` - Admin dashboard
- `GET /admin/problems` - Manage problems
- `GET /admin/submissions` - View all submissions
- `GET /admin/users` - Manage users
- `GET /admin/feedback` - View user feedback
- `GET /admin/api-keys` - Manage API keys

### Admin API (Bearer Token Auth)

- `GET /api/admin/tournaments` - Get tournaments
- `POST /api/admin/problems` - Create problem via API
- `POST /api/admin/problems/{slug}/test-cases` - Add test cases
- `POST /api/admin/problems/{slug}/publish` - Toggle publish status

## Security Features

- **Password Hashing**: Argon2id for secure password storage
- **Session Management**: Secure, HTTP-only cookies with SameSite protection
- **Rate Limiting**: 100 requests/minute per IP (custom token bucket)
- **Input Validation**: Username, email, and password validation
- **SQL Injection Prevention**: Parameterized queries throughout
- **CSP Headers**: Content Security Policy support via tower-http

## Database Schema

### Core Tables

- `users` - User accounts with roles
- `problems` - Code golf challenges
- `test_cases` - Input/output test cases
- `languages` - Supported programming languages
- `submissions` - Code submissions
- `best_submissions` - Best submission per user/problem
- `tournaments` - Competition tournaments
- `tournament_problems` - Problems in tournaments
- `feedback` - User feedback
- `api_keys` - Admin API credentials
- `sessions` - User sessions (SQLite-backed)

### Performance Indexes

- `idx_submissions_status` - Fast status filtering
- `idx_best_submissions_user` - Optimized user queries
- `idx_tournaments_created` - Tournament ordering
- `idx_problems_published` - Published problem filtering
- `idx_problems_tournament` - Tournament lookups

## Testing

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_problem_creation
```

### Test Structure

```
tests/
├── integration/
│   ├── auth_test.rs        # Authentication tests
│   ├── submissions_test.rs # Submission tests
│   ├── problems_test.rs    # Problem tests
│   └── helpers/            # Test utilities
└── unit/                   # Unit tests
```

## Deployment

See [DEPLOYMENT.md](./DEPLOYMENT.md) for comprehensive deployment instructions including:

- Docker Compose deployment
- PostgreSQL migration
- HTTPS/SSL configuration
- Backup strategies
- Monitoring setup
- Scaling recommendations

## Configuration Constants

The application uses configuration constants for maintainability:

```rust
// src/config.rs
pub const DEFAULT_SESSION_EXPIRY_DAYS: i64 = 7;
pub const DEFAULT_MAX_CODE_SIZE: usize = 65536;
pub const DEFAULT_TIME_LIMIT_MS: i64 = 5000;
pub const DEFAULT_MEMORY_LIMIT_KB: i64 = 65536;
```

These can be overridden via environment variables.

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Development Guidelines

- Follow Rust coding conventions
- Use `cargo fmt` for formatting
- Run `cargo clippy` before committing
- Add tests for new features
- Update documentation as needed

## Known Limitations

- **SQLite**: Single-writer lock limits concurrent writes. Use PostgreSQL for production.
- **Code Execution**: Current sandbox uses temp files. Consider WASM or containerization for stronger isolation.
- **Caching**: In-memory cache only. Redis recommended for production.
- **No Job Queue**: Submissions judged via tokio::spawn. No retry/queue mechanism.

## Future Roadmap

- [ ] PostgreSQL database support
- [ ] Redis session store
- [ ] Redis caching layer
- [ ] WebSocket for real-time updates
- [ ] Prometheus metrics
- [ ] Automated backup scripts
- [ ] WASM sandbox for code execution
- [ ] Advanced problem search and filtering
- [ ] Team/duo competition support
- [ ] Custom test case uploads

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Built with [Axum](https://github.com/tokio-rs/axum) web framework
- Database access via [SQLx](https://github.com/launchbadge/sqlx)
- Templates via [MiniJinja](https://github.com/mitsuhiko/minijinja)
- Session management via [tower-sessions](https://github.com/tower-rs/tower-sessions)
- Inspired by [Code Golf Stack Exchange](https://codegolf.stackexchange.com/)

---

**Version**: 0.1.0  
**Last Updated**: 2026-04-27
