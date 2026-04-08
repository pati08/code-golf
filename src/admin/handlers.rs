use axum::{
    Form,
    extract::{Path, State},
    response::{Html, Redirect},
};
use serde::Deserialize;
use sqlx::Row;

use crate::{app::AppState, auth::RequiredAdmin, error::AppError};

fn admin_ctx(admin: &crate::auth::CurrentUser) -> minijinja::value::Value {
    minijinja::context! {
        id => admin.id,
        username => admin.username,
        is_admin => admin.is_admin,
    }
}

pub async fn get_dashboard(
    State(state): State<AppState>,
    RequiredAdmin(admin): RequiredAdmin,
) -> Result<Html<String>, AppError> {
    let problem_count: i64 = sqlx::query("SELECT COUNT(*) FROM problems")
        .fetch_one(&state.db)
        .await?
        .get(0);
    let user_count: i64 = sqlx::query("SELECT COUNT(*) FROM users")
        .fetch_one(&state.db)
        .await?
        .get(0);
    let submission_count: i64 = sqlx::query("SELECT COUNT(*) FROM submissions")
        .fetch_one(&state.db)
        .await?
        .get(0);

    let ctx = minijinja::context! {
        problem_count,
        user_count,
        submission_count,
        current_user => admin_ctx(&admin),
    };
    crate::app::render(&state.templates, "admin/dashboard.html", ctx)
}

pub async fn get_admin_problems(
    State(state): State<AppState>,
    RequiredAdmin(admin): RequiredAdmin,
) -> Result<Html<String>, AppError> {
    let rows = sqlx::query(
        "SELECT id, slug, title, difficulty, is_published, created_at FROM problems ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await?;

    let problems: Vec<_> = rows
        .iter()
        .map(|r| {
            minijinja::context! {
                id => r.get::<i64, _>("id"),
                slug => r.get::<String, _>("slug"),
                title => r.get::<String, _>("title"),
                difficulty => r.get::<String, _>("difficulty"),
                is_published => r.get::<i64, _>("is_published") != 0,
                created_at => r.get::<String, _>("created_at"),
            }
        })
        .collect();

    let ctx = minijinja::context! { problems, current_user => admin_ctx(&admin) };
    crate::app::render(&state.templates, "admin/problems/list.html", ctx)
}

pub async fn get_new_problem(
    State(state): State<AppState>,
    RequiredAdmin(admin): RequiredAdmin,
) -> Result<Html<String>, AppError> {
    let ctx = minijinja::context! { current_user => admin_ctx(&admin) };
    crate::app::render(&state.templates, "admin/problems/new.html", ctx)
}

#[derive(Deserialize)]
pub struct ProblemForm {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub difficulty: String,
    pub time_limit_ms: i64,
    pub memory_limit_kb: i64,
    #[serde(default)]
    pub par_solution: String,
}

pub async fn post_create_problem(
    State(state): State<AppState>,
    RequiredAdmin(admin): RequiredAdmin,
    Form(form): Form<ProblemForm>,
) -> Result<Redirect, AppError> {
    let par_solution: Option<&str> = if form.par_solution.trim().is_empty() {
        None
    } else {
        Some(&form.par_solution)
    };
    let par_byte_count: Option<i64> = par_solution.map(|s| s.trim_end_matches('\n').len() as i64);

    sqlx::query(
        "INSERT INTO problems (slug, title, description, difficulty, time_limit_ms, memory_limit_kb, created_by, par_solution, par_byte_count) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&form.slug)
    .bind(&form.title)
    .bind(&form.description)
    .bind(&form.difficulty)
    .bind(form.time_limit_ms)
    .bind(form.memory_limit_kb)
    .bind(admin.id)
    .bind(par_solution)
    .bind(par_byte_count)
    .execute(&state.db)
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            AppError::BadRequest("Slug already exists".to_string())
        } else {
            AppError::Database(e)
        }
    })?;

    Ok(Redirect::to(&format!(
        "/admin/problems/{}/test-cases",
        form.slug
    )))
}

pub async fn get_edit_problem(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    RequiredAdmin(admin): RequiredAdmin,
) -> Result<Html<String>, AppError> {
    let row = sqlx::query(
        "SELECT id, slug, title, description, difficulty, is_published, time_limit_ms, memory_limit_kb, par_solution, par_byte_count FROM problems WHERE slug = ?",
    )
    .bind(&slug)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let ctx = minijinja::context! {
        problem => minijinja::context! {
            id => row.get::<i64, _>("id"),
            slug => row.get::<String, _>("slug"),
            title => row.get::<String, _>("title"),
            description => row.get::<String, _>("description"),
            difficulty => row.get::<String, _>("difficulty"),
            is_published => row.get::<i64, _>("is_published") != 0,
            time_limit_ms => row.get::<i64, _>("time_limit_ms"),
            memory_limit_kb => row.get::<i64, _>("memory_limit_kb"),
            par_solution => row.get::<Option<String>, _>("par_solution"),
            par_byte_count => row.get::<Option<i64>, _>("par_byte_count"),
        },
        current_user => admin_ctx(&admin),
    };
    crate::app::render(&state.templates, "admin/problems/edit.html", ctx)
}

pub async fn post_update_problem(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    RequiredAdmin(_admin): RequiredAdmin,
    Form(form): Form<ProblemForm>,
) -> Result<Redirect, AppError> {
    let par_solution: Option<&str> = if form.par_solution.trim().is_empty() {
        None
    } else {
        Some(&form.par_solution)
    };
    let par_byte_count: Option<i64> = par_solution.map(|s| s.trim_end_matches('\n').len() as i64);

    sqlx::query(
        "UPDATE problems SET slug = ?, title = ?, description = ?, difficulty = ?, time_limit_ms = ?, memory_limit_kb = ?, par_solution = ?, par_byte_count = ?, updated_at = datetime('now') WHERE slug = ?",
    )
    .bind(&form.slug)
    .bind(&form.title)
    .bind(&form.description)
    .bind(&form.difficulty)
    .bind(form.time_limit_ms)
    .bind(form.memory_limit_kb)
    .bind(par_solution)
    .bind(par_byte_count)
    .bind(&slug)
    .execute(&state.db)
    .await?;

    Ok(Redirect::to("/admin/problems"))
}

pub async fn post_toggle_publish(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    RequiredAdmin(_admin): RequiredAdmin,
) -> Result<Redirect, AppError> {
    sqlx::query("UPDATE problems SET is_published = NOT is_published WHERE slug = ?")
        .bind(&slug)
        .execute(&state.db)
        .await?;
    Ok(Redirect::to("/admin/problems"))
}

pub async fn get_test_cases(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    RequiredAdmin(admin): RequiredAdmin,
) -> Result<Html<String>, AppError> {
    let problem_row = sqlx::query("SELECT id, title, is_published FROM problems WHERE slug = ?")
        .bind(&slug)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let problem_id: i64 = problem_row.get("id");
    let problem_title: String = problem_row.get("title");
    let is_published: bool = problem_row.get::<i64, _>("is_published") != 0;

    let tc_rows = sqlx::query(
        "SELECT id, input, expected_output, is_sample, ordinal FROM test_cases WHERE problem_id = ? ORDER BY ordinal, id",
    )
    .bind(problem_id)
    .fetch_all(&state.db)
    .await?;

    let test_cases: Vec<_> = tc_rows
        .iter()
        .map(|r| {
            minijinja::context! {
                id => r.get::<i64, _>("id"),
                input => r.get::<String, _>("input"),
                expected_output => r.get::<String, _>("expected_output"),
                is_sample => r.get::<i64, _>("is_sample") != 0,
                ordinal => r.get::<i64, _>("ordinal"),
            }
        })
        .collect();

    let ctx = minijinja::context! {
        problem => minijinja::context! { id => problem_id, slug, title => problem_title, is_published },
        test_cases,
        current_user => admin_ctx(&admin),
    };
    crate::app::render(&state.templates, "admin/problems/test_cases.html", ctx)
}

#[derive(Deserialize)]
pub struct TestCaseForm {
    pub input: String,
    pub expected_output: String,
    pub is_sample: Option<String>,
    pub ordinal: i64,
}

pub async fn post_add_test_case(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    RequiredAdmin(_admin): RequiredAdmin,
    Form(form): Form<TestCaseForm>,
) -> Result<Redirect, AppError> {
    let problem_row = sqlx::query("SELECT id FROM problems WHERE slug = ?")
        .bind(&slug)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let problem_id: i64 = problem_row.get("id");

    let is_sample = form.is_sample.is_some() as i64;
    sqlx::query(
        "INSERT INTO test_cases (problem_id, input, expected_output, is_sample, ordinal) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(problem_id)
    .bind(&form.input)
    .bind(&form.expected_output)
    .bind(is_sample)
    .bind(form.ordinal)
    .execute(&state.db)
    .await?;

    Ok(Redirect::to(&format!("/admin/problems/{slug}/test-cases")))
}

pub async fn post_delete_test_case(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    RequiredAdmin(_admin): RequiredAdmin,
) -> Result<Redirect, AppError> {
    let row = sqlx::query(
        "SELECT p.slug FROM test_cases tc JOIN problems p ON p.id = tc.problem_id WHERE tc.id = ?",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let slug: String = row.get("slug");

    sqlx::query("DELETE FROM test_cases WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;

    Ok(Redirect::to(&format!("/admin/problems/{slug}/test-cases")))
}

pub async fn get_admin_submissions(
    State(state): State<AppState>,
    RequiredAdmin(admin): RequiredAdmin,
) -> Result<Html<String>, AppError> {
    let rows = sqlx::query(
        r#"SELECT s.id, s.status, s.byte_count, s.created_at,
               u.username, p.title as problem_title, p.slug as problem_slug,
               l.display_name as language_name
           FROM submissions s
           JOIN users u ON u.id = s.user_id
           JOIN problems p ON p.id = s.problem_id
           JOIN languages l ON l.id = s.language_id
           ORDER BY s.created_at DESC
           LIMIT 100"#,
    )
    .fetch_all(&state.db)
    .await?;

    let submissions: Vec<_> = rows
        .iter()
        .map(|r| {
            minijinja::context! {
                id => r.get::<i64, _>("id"),
                status => r.get::<String, _>("status"),
                byte_count => r.get::<i64, _>("byte_count"),
                created_at => r.get::<String, _>("created_at"),
                username => r.get::<String, _>("username"),
                problem_title => r.get::<String, _>("problem_title"),
                problem_slug => r.get::<String, _>("problem_slug"),
                language_name => r.get::<String, _>("language_name"),
            }
        })
        .collect();

    let ctx = minijinja::context! { submissions, current_user => admin_ctx(&admin) };
    crate::app::render(&state.templates, "admin/submissions/list.html", ctx)
}

pub async fn get_admin_users(
    State(state): State<AppState>,
    RequiredAdmin(admin): RequiredAdmin,
) -> Result<Html<String>, AppError> {
    let rows = sqlx::query(
        "SELECT id, username, email, is_admin, created_at FROM users ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await?;

    let users: Vec<_> = rows
        .iter()
        .map(|r| {
            minijinja::context! {
                id => r.get::<i64, _>("id"),
                username => r.get::<String, _>("username"),
                email => r.get::<String, _>("email"),
                is_admin => r.get::<i64, _>("is_admin") != 0,
                created_at => r.get::<String, _>("created_at"),
            }
        })
        .collect();

    let ctx = minijinja::context! { users, current_user => admin_ctx(&admin) };
    crate::app::render(&state.templates, "admin/users/list.html", ctx)
}

pub async fn post_toggle_admin(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    RequiredAdmin(admin): RequiredAdmin,
) -> Result<Redirect, AppError> {
    if id == admin.id {
        return Err(AppError::BadRequest(
            "Cannot change your own admin status".to_string(),
        ));
    }
    sqlx::query("UPDATE users SET is_admin = NOT is_admin WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(Redirect::to("/admin/users"))
}
