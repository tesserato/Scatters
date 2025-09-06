use crate::error::AppError;
use crate::processing::PlotData;
// Removed hypertext dependency - using simple string formatting
use polars::prelude::*;
use serde_json::Value;

/// Generates a self-contained HTML file with an interactive ECharts plot.
pub fn generate_html_plot(plot_data: &PlotData) -> Result<String, AppError> {
    // Convert Polars Series into a format suitable for ECharts JSON
    let series_json_objects = build_series_json(plot_data)?;

    // Determine x-axis type based on data
    let x_axis_type = match plot_data.x_series.dtype() {
        DataType::Datetime(_, _) | DataType::Date => "time",
        DataType::String => "category",
        _ => "value",
    };

    // Compute y-axis limits from data
    let (y_min_str, y_max_str) = {
        let mut min_v = f64::INFINITY;
        let mut max_v = f64::NEG_INFINITY;
        for ys in &plot_data.y_series_list {
            if let Ok(casted) = ys.cast(&DataType::Float64) {
                let ca = casted.f64().unwrap();
                if let Some(mn) = ca.min() { if mn.is_finite() { min_v = min_v.min(mn); } }
                if let Some(mx) = ca.max() { if mx.is_finite() { max_v = max_v.max(mx); } }
            }
        }
        if min_v.is_finite() && max_v.is_finite() {
            let span = (max_v - min_v).abs();
            let pad = if span == 0.0 { 1.0 } else { span * 0.05 };
            (format!("{}", min_v - pad), format!("{}", max_v + pad))
        } else {
            (String::from("null"), String::from("null"))
        }
    };

    // Generate HTML using simple string formatting
    let html_content = format!(r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>{}</title>
    <script src="https://cdn.jsdelivr.net/npm/echarts/dist/echarts.min.js"></script>
</head>
<body>
    <div id="main" style="width: 100%; height: 95vh;"></div>
    <script>
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
            xAxis: {{ type: '{}', splitLine: {{ show: false }} }},
            yAxis: {{ type: 'value', axisLine: {{ show: true }}, min: {}, max: {} }},
            dataZoom: [
                {{ type: 'inside', start: 0, end: 100 }},
                {{ type: 'slider', start: 0, end: 100, height: 40 }}
            ],
            series: [ {} ]
        }});
        // Helper to compute size for visible window
function computeSize(n, pct) {{
            pct = Math.max(0, Math.min(1, pct));
            var visibleN = (pct <= 0) ? 1 : Math.max(1, Math.round(n * pct));
return Math.max(1, Math.min(36, (14 - Math.log10(visibleN + 1) * 3.5) * 2));
        }}

        // Apply sizes immediately and on zoom
function applySymbolSizes(pct) {{
            var opt = myChart.getOption();
            var series = opt.series || [];
var newSeries = series.map(function (s) {{
                var n = (s.metaN != null) ? s.metaN : ((s.data && s.data.length) ? s.data.length : 1000);
                var size = computeSize(n, pct);
                // Toggle large mode based on visible points for better styling accuracy when zoomed-in
                var visibleN = Math.max(1, Math.round(n * pct));
                var useLarge = visibleN > 5000;
return {{ symbolSize: size, large: useLarge }};
}});
            myChart.setOption({{ series: newSeries }}, false, false);
        }}

// Initial apply for full view
        applySymbolSizes(1.0);

        // Adapt symbol sizes to the current zoom window
myChart.on('dataZoom', function () {{
            var opt = myChart.getOption();
            var dzArr = opt.dataZoom || [];
if (!dzArr.length) {{ return; }}
            var dz = dzArr[0];
            var start = (dz.start != null) ? dz.start : 0;
            var end = (dz.end != null) ? dz.end : 100;
            var pct = Math.max(0, (end - start) / 100);
applySymbolSizes(pct);
        }});
        // Re-apply sizes after toolbox restore resets options
        myChart.on('restore', function () {{
            setTimeout(function() {{ applySymbolSizes(1.0); }}, 0);
        }});
    </script>
</body>
</html>"#, 
        plot_data.title, 
        plot_data.title, 
        x_axis_type,
        y_min_str,
        y_max_str,
        series_json_objects.join(",")
    );

    Ok(html_content)
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
                if !matches!(x_val, AnyValue::Null) && !matches!(y_val, AnyValue::Null) {
                    let x = any_value_to_json_value(x_val);
                    let y = any_value_to_json_value(y_val);
                    Some([x, y])
                } else {
                    None
                }
            })
            .collect();

        let data_json = serde_json::to_string(&data_points)?;
        let n_points = data_points.len();

        let series_obj = format!(
            r#"{{
                name: '{}',
                type: 'scatter',
                metaN: {},
                symbolSize: (function() {{
                    const n = {};
                    return function(/* value, params */) {{
// Larger for small n, smaller for large n; hard minimum of 1px, cap ~36px
return Math.max(1, Math.min(36, (14 - Math.log10(n + 1) * 3.5) * 2));
                    }}
                }})(),
                large: true,
                largeThreshold: 2000,
                data: {}
            }}"#,
            y_series.name(),
            n_points,
            n_points,
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
