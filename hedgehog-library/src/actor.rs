use crate::datasource::{DataProvider, ListQuery, QueryError, QueryHandler, SqliteDataProvider};
use actix::{Actor, Context, Handler, Message};

#[derive(Message)]
#[rtype(result = "Result<Vec<Q::Item>, QueryError>")]
pub struct QueryRequest<Q: ListQuery> {
    pub data: Q,
    pub count: usize,
    pub offset: usize,
}

#[derive(Message)]
#[rtype(result = "Result<usize, QueryError>")]
pub struct SizeRequest<Q: ListQuery>(Q);

pub struct Library<D: DataProvider = SqliteDataProvider> {
    data_provider: D,
}

impl<D: DataProvider> Library<D> {
    pub fn new(data_provider: D) -> Self {
        Library { data_provider }
    }
}

impl<D: DataProvider + 'static> Actor for Library<D> {
    type Context = Context<Self>;
}

impl<D, Q> Handler<QueryRequest<Q>> for Library<D>
where
    D: DataProvider + QueryHandler<Q> + 'static,
    Q: ListQuery,
{
    type Result = Result<Vec<Q::Item>, QueryError>;

    fn handle(&mut self, msg: QueryRequest<Q>, _ctx: &mut Self::Context) -> Self::Result {
        self.data_provider.query(msg.data, msg.offset, msg.count)
    }
}

impl<D, Q> Handler<SizeRequest<Q>> for Library<D>
where
    D: DataProvider + QueryHandler<Q> + 'static,
    Q: ListQuery,
{
    type Result = Result<usize, QueryError>;

    fn handle(&mut self, msg: SizeRequest<Q>, _ctx: &mut Self::Context) -> Self::Result {
        self.data_provider.get_size(msg.0)
    }
}
