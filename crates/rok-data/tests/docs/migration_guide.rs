//! `docs/migration-guide.md`'s samples, compiled.

/// The row-level writes and the chain additions.
mod writes_and_chains {
    use rok_data::prelude::*;

    #[derive(Debug, Clone, Model)]
    struct User {
        #[rok(generated)]
        id: i64,
        email: String,
        role: String,
    }

    async fn archive(db: &Db, user: User) -> rok_data::Result<()> {
        user.delete(db).await
    }

    async fn renamed(db: Db) -> rok_data::Result<()> {
        let user = User::insert_builder()
            .set(User::EMAIL, "a@b.c")
            .exec(&db)
            .await?;
        user.update(&db).await?;
        Ok(())
    }

    async fn chain_additions(db: Db) -> rok_data::Result<()> {
        // `where` is a Rust keyword, so `where_` is the spelling; `filter`
        // stays the canonical verb and the generated code keeps using it.
        let admins = User::where_(User::ROLE.eq("admin")).all(&db).await?;

        // one row or many, without fetching them
        if User::exists(&db).await? {
            // …
        }

        // the same rows as `all`, as a `ModelCollection`
        let users = User::query().get(&db).await?;

        // the rows in windows of 100 — `db` as a reference the `async move`
        // blocks copy
        let db = &db;
        User::query()
            .chunk(db, 100, |rows| async move {
                for user in rows {
                    archive(db, user).await?;
                }
                Ok(())
            })
            .await?;
        Ok(())
    }
}

/// Relations on the model.
mod relations {
    use rok_data::prelude::*;

    #[derive(Debug, Clone, Model)]
    #[rok(has_many(posts = Post::AUTHOR_ID))]
    struct User {
        #[rok(generated)]
        id: i64,
    }

    #[derive(Debug, Clone, Model)]
    struct Post {
        #[rok(generated)]
        id: i64,
        #[rok(belongs_to(author = User))]
        author_id: i64,
        draft: bool,
    }

    async fn accessors(db: Db, post: Post, user: User) -> rok_data::Result<()> {
        // a post belongs to its author; a user has many posts
        post.author().all(&db).await?;
        user.posts().where_(Post::DRAFT.eq(false)).all(&db).await?;
        Ok(())
    }
}

/// Named scopes.
mod scopes {
    use rok_data::prelude::*;

    #[derive(Model)]
    #[rok(scopes(popular = popular, active = active))]
    struct User {
        id: i64,
        views: i32,
    }

    // a scope is a function from a query chain to a query chain
    fn popular(query: Select<User>) -> Select<User> {
        query.filter(User::VIEWS.gt(100))
    }

    fn active(query: Select<User>) -> Select<User> {
        query.filter(User::VIEWS.gt(0))
    }

    fn calls() {
        // the model's entry
        let popular = User::popular();
        // the chain's method — the `UserScopes` trait the derive writes next to
        // the model; generated code re-exports it from `crate::db::prelude`
        let popular_and_active = User::popular().active();
    }
}

/// A parameterized scope.
mod parameterized {
    use rok_data::prelude::*;

    #[derive(Model)]
    #[rok(scopes(older_than(n: i32) = older_than))]
    struct User {
        id: i64,
        age: i32,
    }

    // the free function takes the query, then the declared parameters
    fn older_than(query: Select<User>, n: i32) -> Select<User> {
        query.filter(User::AGE.gt(n))
    }

    fn calls() {
        // the entry and the chain method take the same shape
        let adults = User::older_than(18);
        let seniors = User::query().older_than(65);
    }
}
