use serenity::prelude::{
    TypeMap,
    TypeMapKey,
};
use tokio::sync::RwLock;

pub(crate) async fn get_value<T>(map: &RwLock<TypeMap>) -> T::Value
where
    T: TypeMapKey,
    T::Value: Clone,
{
    let lock = map.read().await;
    lock.get::<T>().unwrap().clone()
}
