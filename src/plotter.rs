use crate::error::AppError;
use crate::processing::PlotData;
use askama::Template;
use polars::prelude::*;
use serde_json::Value;

// plot_data.title,
// autoscale_js,
// animations_js,
// max_decimals_js,
// use_white_js,
// plot_data.title,
// x_axis_type,
// x_axis_label_extra,
// y_min_str,
// y_max_str,
// series_json_objects.join(",")

#[derive(Template)]
#[template(path = "page.html")] // using the template in this path, relative
struct HelloTemplate<'a> {
    title: &'a str,
    autoscale_y: &'a bool,
    animations: &'a bool,
    max_decimals: &'a i32,
    use_white_theme: &'a bool,
    x_axis_type: &'a str,
    x_axis_label_extra: &'a str,
    y_min: &'a f32,
    y_max: &'a f32,
    series_json: &'a str,
}

/// Generates a self-contained HTML file with an interactive ECharts plot.
pub fn generate_html_plot(plot_data: &PlotData) -> Result<String, askama::Error> {
    // Convert Polars Series into a format suitable for ECharts JSON
    let series_json_objects = build_series_json(plot_data).unwrap();

    // Determine x-axis type based on data
    let x_axis_type = match plot_data.x_series.dtype() {
        DataType::Datetime(_, _) | DataType::Date => "time",
        DataType::String => "category",
        _ => "value",
    };
    // Extra formatter only for numeric X axis; color is always set at option-level.
    let x_axis_label_extra = if x_axis_type == "value" {
        ", formatter: formatNumber"
    } else {
        ""
    };

    // Compute y-axis limits from data
    let (y_min_str, y_max_str) = {
        let mut min_v = f64::INFINITY;
        let mut max_v = f64::NEG_INFINITY;
        for ys in &plot_data.y_series_list {
            if let Ok(casted) = ys.cast(&DataType::Float64) {
                let ca = casted.f64().unwrap();
                if let Some(mn) = ca.min() {
                    if mn.is_finite() {
                        min_v = min_v.min(mn);
                    }
                }
                if let Some(mx) = ca.max() {
                    if mx.is_finite() {
                        max_v = max_v.max(mx);
                    }
                }
            }
        }
        if min_v.is_finite() && max_v.is_finite() {
            let span = (max_v - min_v).abs();
            let pad = if span == 0.0 { 1.0 } else { span * 0.10 };
            (format!("{}", min_v - pad), format!("{}", max_v + pad))
        } else {
            (String::from("null"), String::from("null"))
        }
    };

    // Generate HTML using simple string formatting
    // let autoscale_js = if plot_data.autoscale_y {
    //     "true"
    // } else {
    //     "false"
    // };
    // let animations_js = if plot_data.animations {
    //     "true"
    // } else {
    //     "false"
    // };
    // let max_decimals_js = plot_data.max_decimals;
    // let use_white_js = if plot_data.use_white_theme {
    //     "true"
    // } else {
    //     "false"
    // };
    let html_content: Result<String, askama::Error> = HelloTemplate {
        title: &plot_data.title,
        autoscale_y: &plot_data.autoscale_y,
        animations: &plot_data.animations,
        max_decimals: &plot_data.max_decimals,
        use_white_theme: &plot_data.use_white_theme,
        x_axis_type: &x_axis_type,
        x_axis_label_extra: &x_axis_label_extra,
        y_min: &y_min_str.parse::<f32>().unwrap(),
        y_max: &y_max_str.parse::<f32>().unwrap(),
        series_json: &series_json_objects.join(","),
    }
    .render();

    html_content
}

/// Builds the JavaScript object strings for each series.
fn build_series_json(plot_data: &PlotData) -> Result<Vec<String>, AppError> {
    let mut series_objects = Vec::new();
    let x_series = &plot_data.x_series;

    for y_series in &plot_data.y_series_list {
        // Zip X and Y series into [x, y] pairs, filtering out nulls
        let mut data_points: Vec<[Value; 2]> = Vec::new();
        let mut mark_lines_data: Vec<Value> = Vec::new();
        let mut x_min = f64::INFINITY;
        let mut x_max = f64::NEG_INFINITY;
        let mut y_min = f64::INFINITY;
        let mut y_max = f64::NEG_INFINITY;

        for (x_val, y_val) in x_series.iter().zip(y_series.iter()) {
            if !matches!(x_val, AnyValue::Null) {
                if let AnyValue::String(s) = y_val {
                    if s == "|" {
                        // Create a markLine for the vertical line
                        let color = "#c23531";
                        mark_lines_data.push(serde_json::json!({
                            "xAxis": any_value_to_json_value(x_val.clone()),
                            "lineStyle": {
                                "color": color,
                                "width": 2,
                                "type": "solid",
                            },
                            "symbol": "none"
                        }));
                        continue; // Skip adding to data_points
                    }
                }

                if !matches!(y_val, AnyValue::Null) {
                    // JSON values for rendering
                    let x_json = any_value_to_json_value(x_val.clone());
                    let y_json = any_value_to_json_value(y_val.clone());
                    data_points.push([x_json, y_json]);

                    // Numeric values for meta range calculations (only numeric/time)
                    if let (Some(xn), Some(yn)) = (any_value_to_f64(x_val), any_value_to_f64(y_val))
                    {
                        if xn.is_finite() {
                            x_min = x_min.min(xn);
                            x_max = x_max.max(xn);
                        }
                        if yn.is_finite() {
                            y_min = y_min.min(yn);
                            y_max = y_max.max(yn);
                        }
                    }
                }
            }
        }

        let n_points = data_points.len();
        let x_min_str = if x_min.is_finite() {
            format!("{}", x_min)
        } else {
            "null".to_string()
        };
        let x_max_str = if x_max.is_finite() {
            format!("{}", x_max)
        } else {
            "null".to_string()
        };
        let y_min_str = if y_min.is_finite() {
            format!("{}", y_min)
        } else {
            "null".to_string()
        };
        let y_max_str = if y_max.is_finite() {
            format!("{}", y_max)
        } else {
            "null".to_string()
        };

        let series_obj = serde_json::json!({
            "name": y_series.name(),
            "type": "scatter",
            "metaN": n_points,
            "metaXMin": x_min_str,
            "metaXMax": x_max_str,
            "metaYMin": y_min_str,
            "metaYMax": y_max_str,
            "symbolSize": 10,
            "large": true,
            "largeThreshold": 2000,
            "data": data_points,
            "markLine": { "data": mark_lines_data, "symbol": "none" }
        });

        let series_obj_str = serde_json::to_string(&series_obj)?;
        series_objects.push(series_obj_str);
    }
    Ok(series_objects)
}

/// Converts a Polars AnyValue to a serde_json::Value.
fn any_value_to_json_value(av: AnyValue) -> Value {
    match av {
        AnyValue::Null => Value::Null,
        AnyValue::Boolean(b) => Value::Bool(b),
        AnyValue::String(s) => Value::String(s.to_string()),
        AnyValue::UInt8(v) => v.into(),
        AnyValue::UInt16(v) => v.into(),
        AnyValue::UInt32(v) => v.into(),
        AnyValue::UInt64(v) => v.into(),
        AnyValue::Int8(v) => v.into(),
        AnyValue::Int16(v) => v.into(),
        AnyValue::Int32(v) => v.into(),
        AnyValue::Int64(v) => v.into(),
        AnyValue::Float32(v) => v.into(),
        AnyValue::Float64(v) => v.into(),
        // Polars Date is days since epoch
        AnyValue::Date(days) => {
            let ms = (days as i64) * 86_400_000;
            ms.into()
        }
        // Polars Datetime is epoch value with unit
        AnyValue::Datetime(v, unit, _) => {
            let ms = match unit {
                polars::prelude::TimeUnit::Nanoseconds => v / 1_000_000,
                polars::prelude::TimeUnit::Microseconds => v / 1_000,
                polars::prelude::TimeUnit::Milliseconds => v,
            };
            ms.into()
        }
        _ => Value::String(av.to_string()), // Fallback for other types
    }
}

/// Convert AnyValue to f64 if numeric or datetime/date; otherwise None.
fn any_value_to_f64(av: AnyValue) -> Option<f64> {
    match av {
        AnyValue::UInt8(v) => Some(v as f64),
        AnyValue::UInt16(v) => Some(v as f64),
        AnyValue::UInt32(v) => Some(v as f64),
        AnyValue::UInt64(v) => Some(v as f64),
        AnyValue::Int8(v) => Some(v as f64),
        AnyValue::Int16(v) => Some(v as f64),
        AnyValue::Int32(v) => Some(v as f64),
        AnyValue::Int64(v) => Some(v as f64),
        AnyValue::Float32(v) => Some(v as f64),
        AnyValue::Float64(v) => Some(v),
        AnyValue::Date(days) => Some((days as i64 as f64) * 86_400_000.0),
        AnyValue::Datetime(v, unit, _) => Some(match unit {
            polars::prelude::TimeUnit::Nanoseconds => (v as f64) / 1_000_000.0,
            polars::prelude::TimeUnit::Microseconds => (v as f64) / 1_000.0,
            polars::prelude::TimeUnit::Milliseconds => v as f64,
        }),
        _ => None,
    }
}
