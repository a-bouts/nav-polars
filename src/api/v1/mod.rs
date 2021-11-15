use rocket::{delete, get, post, put, Route, routes, State};
use rocket::form::{FromForm, FromFormField};
use rocket::http::Status;
use rocket::serde::json::Json;

use crate::polar::{Polar, PolarError, PolarService};

pub(crate) fn routes() -> Vec<Route> {
    routes![list, get, find_by_polar_id, post, put, delete, archive, restore]
}

#[derive(FromForm)]
struct Sort {
    sort_by: String,
    #[field(default = Order::Asc)]
    order: Order,
}

#[derive(FromFormField)]
enum Order {
    Asc,
    Desc
}

#[get("/polars?<archived>&<sort..>", rank = 25)]
async fn list(polar_service: &State<PolarService>, archived: Option<bool>, sort: Option<Sort>) -> Result<Json<Vec<Polar>>, Status> {

    match polar_service.list(archived).await {
        Ok(polars) => {
            let mut polars: Vec<Polar> = polars.into_iter().map(|r| r.into()).collect();
            if let Some(sort) = sort {
                polars.sort_by(|a, b| {
                    let (a, b) = match sort.order {
                        Order::Asc => (a, b),
                        Order::Desc => (b, a)
                    };

                    match sort.sort_by.as_str() {
                        "id" => a.id.cmp(&b.id),
                        "_id" => a.polar_id.cmp(&b.polar_id),
                        _ => a.id.cmp(&b.id),
                    }
                })
            }

            Ok(Json(polars))
        },
        Err(_) => Err(Status::InternalServerError)
    }
}

#[get("/polars/<polar_id>")]
async fn get(polar_service: &State<PolarService>, polar_id: String) -> Result<Json<Polar>, Status> {

    match polar_service.get(polar_id).await {
        Ok(None) => Err(Status::NotFound),
        Ok(Some(polar)) => Ok(Json(polar.into())),
        Err(_) => Err(Status::InternalServerError)
    }
}

#[get("/polars?<polar_id>")]
async fn find_by_polar_id(polar_service: &State<PolarService>, polar_id: u8) -> Result<Json<Polar>, Status> {

    match polar_service.find_by_polar_id(polar_id).await {
        Ok(None) => Err(Status::NotFound),
        Ok(Some(polar)) => Ok(Json(polar.into())),
        Err(_) => Err(Status::InternalServerError)
    }
}

#[post("/polars", data = "<polar>")]
async fn post(polar_service: &State<PolarService>, polar: Json<Polar>) -> Status {

    let mut polar = polar.into_inner();
    if polar.id.is_none() {
        polar.id = polar.label.split("/").last().map(|x| x.to_string());
    }

    match polar_service.create(&polar).await {
        Ok(_) => Status::Created,
        Err(error) => {
            match error.downcast_ref::<PolarError>() {
                Some(PolarError::AlreadyExists(_)) => Status::Conflict,
                Some(PolarError::IdIsMandatory()) => Status::BadRequest,
                _ => Status::InternalServerError,
            }
        }
    }
}

#[post("/polars/<polar_id>/archive")]
async fn archive(polar_service: &State<PolarService>, polar_id: String) -> Status {
    match polar_service.archive(polar_id).await {
        Ok(_) => Status::Ok,
        Err(error) => {
            match error.downcast_ref::<PolarError>() {
                Some(PolarError::NotFound(_)) => Status::NotFound,
                _ => Status::InternalServerError,
            }
        }
    }
}

#[post("/polars/<polar_id>/restore")]
async fn restore(polar_service: &State<PolarService>, polar_id: String) -> Status {
    match polar_service.restore(polar_id).await {
        Ok(_) => Status::Created,
        Err(error) => {
            match error.downcast_ref::<PolarError>() {
                Some(PolarError::NotFound(_)) => Status::NotFound,
                Some(PolarError::AlreadyExists(_)) => Status::Conflict,
                _ => Status::InternalServerError,
            }
        }
    }
}

#[put("/polars/<polar_id>", data = "<polar>")]
async fn put(polar_service: &State<PolarService>, polar_id: String, polar: Json<Polar>) -> Status {

    match polar_service.update(polar_id, &polar.into_inner().into()).await {
        Ok(_) => Status::NoContent,
        Err(error) => {
            match error.downcast_ref::<PolarError>() {
                Some(PolarError::NotFound(_)) => Status::NotFound,
                _ => Status::InternalServerError,
            }
        }
    }
}

#[delete("/polars/<polar_id>")]
async fn delete(polar_service: &State<PolarService>, polar_id: String) -> Status {

    match polar_service.delete(polar_id).await {
        Ok(_) => Status::NoContent,
        Err(error) => {
            match error.downcast_ref::<PolarError>() {
                Some(PolarError::NotFound(_)) => Status::NotFound,
                _ => Status::InternalServerError,
            }
        }
    }
}
