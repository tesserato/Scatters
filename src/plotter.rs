use crate::error::AppError;
use crate::processing::PlotData;
use hypertext::{html, maud, Raw};
use polars::prelude::*;
use serde_json::Value;

/// Generates a self-contained HTML file with an interactive ECharts plot.
pub fn generate_html_plot(plot_data: &PlotData) -> Result<String, AppError> {
    // Convert Polars Series into a format suitable for ECharts JSON
    let series_json_objects = build_series_json(plot_data)?;

    // Use hypertext (maud syntax) to build the HTML structure
    let markup = html! {
        <!DOCTYPE html>
        html {
            head {
                meta(charset="utf-8");
                title { (plot_data.title) }
                script(src="https://cdn.jsdelivr.net/npm/echarts/dist/echarts.min.js") {}
            }
            body {
                div(id="main", style="width: 100%; height: 95vh;") {}
                script {
                    // Use Raw() to inject the JavaScript code without HTML escaping
                    (Raw(format!(r#"
                        var myChart = echarts.init(document.getElementById('main'), 'light');
                        myChart.setOption({{
                            title: {{ text: '{}' }},
                            tooltip: {{ trigger: 'axis', axisPointer: {{ type: 'cross' }} }},
                            legend: {{ type: 'scroll', top: 30 }},
                            grid: {{ left: '5%', right: '5%', bottom: '10%', containLabel: true }},
                            toolbox: {{
                                feature: {{
                                    dataZoom: {{ yAxisIndex: 'none' }},
                                    restore: {{}},
                                    saveAsImage: {{}}
                                }}
                            }},
                            xAxis: {{ type: 'value', splitLine: {{ show: false }} }},
                            yAxis: {{ type: 'value', axisLine: {{ show: true }} }},
                            dataZoom: [
                                {{ type: 'inside', start: 0, end: 100 }},
                                {{ type: 'slider', start: 0, end: 100, height: 40 }}
                            ],
                            series: [ {} ]
                        }});
                    "#, plot_data.title, series_json_objects.join(","))))
                }
            }
        }
    };

    Ok(markup.render())
}

/// Builds the JavaScript object strings for each series.
fn build_series_json(plot_data: &PlotData) -> Result<Vec<String>, AppError> {
    let mut series_objects = Vec::new();
    let x_series = &plot_data.x_series;

    for y_series in &plot_data.y_series_list {
        // Zip X and Y series into [x, y] pairs, filtering out nulls
        let data_points: Vec<[Value; 2]> = x_series
            .iter()
            .zip(y_series.iter())
            .filter_map(|(x_val, y_val)| {
                if x_val.is_not_null() && y_val.is_not_null() {
                    let x = any_value_to_json_value(x_val);
                    let y = any_value_to_json_value(y_val);
                    Some([x, y])
                } else {
                    None
                }
            })
            .collect();

        let data_json = serde_json::to_string(&data_points)?;

        let series_obj = format!(
            r#"{{
                name: '{}',
                type: 'scatter',
                symbolSize: 5,
                large: true,
                sampling: 'lttb',
                data: {}
            }}"#,
            y_series.name(),
            data_json
        );
        series_objects.push(series_obj);
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
        AnyValue::Date(d) => Value::String(d.to_string()),
        AnyValue::Datetime(dt, _, _) => Value::String(dt.to_string()),
        _ => Value::String(av.to_string()), // Fallback for other types
    }
}
