use std::marker::PhantomData;

pub struct New;
pub struct Unmoderated;
pub struct Published;
pub struct Deleted;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Title(pub String);
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Body(pub String);
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Author(pub String);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UtcMillis(pub i64);

#[derive(Debug, Clone)]
pub struct Post<State> {
    pub title: Title,
    pub body: Body,
    pub author: Author,
    pub created_at: UtcMillis,
    pub updated_at: UtcMillis,
    _state: PhantomData<State>,
}

impl Post<New> {
    pub fn new(title: Title, body: Body, author: Author, now: UtcMillis) -> Self {
        Self {
            title,
            body,
            author,
            created_at: now,
            updated_at: now,
            _state: PhantomData,
        }
    }
    pub fn publish(self, when: UtcMillis) -> Post<Unmoderated> {
        Post {
            title: self.title,
            body: self.body,
            author: self.author,
            created_at: self.created_at,
            updated_at: when,
            _state: PhantomData,
        }
    }
}

impl Post<Unmoderated> {
    pub fn allow(self, when: UtcMillis) -> Post<Published> {
        Post {
            title: self.title,
            body: self.body,
            author: self.author,
            created_at: self.created_at,
            updated_at: when,
            _state: PhantomData,
        }
    }
    pub fn deny(self, when: UtcMillis) -> Post<Deleted> {
        Post {
            title: self.title,
            body: self.body,
            author: self.author,
            created_at: self.created_at,
            updated_at: when,
            _state: PhantomData,
        }
    }
}

impl Post<Published> {
    pub fn delete(self, when: UtcMillis) -> Post<Deleted> {
        Post {
            title: self.title,
            body: self.body,
            author: self.author,
            created_at: self.created_at,
            updated_at: when,
            _state: PhantomData,
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_transitions_compile() {
        let p = Post::<New>::new(
            Title("Hello".into()),
            Body("World".into()),
            Author("Alice".into()),
            UtcMillis(1),
        );
        let p = p.publish(UtcMillis(2));
        let _p = p.allow(UtcMillis(3));
    }

    #[test]
    fn unmoderated_deny_to_deleted() {
        let p = Post::<New>::new(
            Title("T".into()),
            Body("B".into()),
            Author("A".into()),
            UtcMillis(1),
        );
        let p = p.publish(UtcMillis(2));
        let _d = p.deny(UtcMillis(3));
    }

    // let p = Post::<New>::new(...).delete(UtcMillis(5)); // ❌ не скомпілюється
    // let d: Post<Deleted> = ...; d.deny(UtcMillis(5));   // ❌ не скомпілюється
}
