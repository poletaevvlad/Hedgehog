use crate::datasource::{DataProvider, ListQuery, PagedQueryHandler, QueryError, QueryHandler};
use crate::sqlite::SqliteDataProvider;
use actix::{Actor, Context, Handler, Message};

#[derive(Message)]
#[rtype(result = "Result<Vec<Q::Item>, QueryError>")]
pub struct PagedQueryRequest<Q: ListQuery> {
    pub data: Q,
    pub count: usize,
    pub offset: usize,
}

impl<Q: ListQuery> PagedQueryRequest<Q> {
    pub fn new(data: Q, count: usize) -> Self {
        PagedQueryRequest {
            data,
            count,
            offset: 0,
        }
    }

    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }
}

#[derive(Message)]
#[rtype(result = "Result<usize, QueryError>")]
pub struct SizeRequest<Q: ListQuery>(pub Q);

#[derive(Message)]
#[rtype(result = "Result<Vec<Q::Item>, QueryError>")]
pub struct QueryRequest<Q: ListQuery>(pub Q);

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

impl<D, Q> Handler<PagedQueryRequest<Q>> for Library<D>
where
    D: DataProvider + PagedQueryHandler<Q> + 'static,
    Q: ListQuery,
{
    type Result = Result<Vec<Q::Item>, QueryError>;

    fn handle(&mut self, msg: PagedQueryRequest<Q>, _ctx: &mut Self::Context) -> Self::Result {
        self.data_provider
            .query_page(msg.data, msg.offset, msg.count)
    }
}

impl<D, Q> Handler<SizeRequest<Q>> for Library<D>
where
    D: DataProvider + PagedQueryHandler<Q> + 'static,
    Q: ListQuery,
{
    type Result = Result<usize, QueryError>;

    fn handle(&mut self, msg: SizeRequest<Q>, _ctx: &mut Self::Context) -> Self::Result {
        self.data_provider.get_size(msg.0)
    }
}

impl<D, Q> Handler<QueryRequest<Q>> for Library<D>
where
    D: DataProvider + QueryHandler<Q> + 'static,
    Q: ListQuery,
{
    type Result = Result<Vec<Q::Item>, QueryError>;

    fn handle(&mut self, msg: QueryRequest<Q>, _ctx: &mut Self::Context) -> Self::Result {
        self.data_provider.query(msg.0)
    }
}
