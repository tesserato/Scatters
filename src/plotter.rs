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
    let x_axis_label_extra = if x_axis_type == "value" { ", axisLabel: { formatter: formatNumber }" } else { "" };

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
            let pad = if span == 0.0 { 1.0 } else { span * 0.10 };
            (format!("{}", min_v - pad), format!("{}", max_v + pad))
        } else {
            (String::from("null"), String::from("null"))
        }
    };

    // Generate HTML using simple string formatting
    let autoscale_js = if plot_data.autoscale_y { "true" } else { "false" };
    let animations_js = if plot_data.animations { "true" } else { "false" };
    let max_decimals_js = plot_data.max_decimals;
    let use_white_js = if plot_data.use_white_theme { "true" } else { "false" };
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
        var AUTOSCALE_Y = {};
        var ANIMATIONS = {};
        var MAX_DECIMALS = {};
        var USE_WHITE = {};
        var THEME = USE_WHITE ? 'white' : 'dark';
        // Register themes (light and dark)
        echarts.registerTheme('white', {{
            backgroundColor: '#ffffff',
            textStyle: {{ color: '#333' }},
            color: ['#4e79a7','#f28e2b','#e15759','#76b7b2','#59a14f','#edc949','#af7aa1','#ff9da7','#9c755f','#bab0ab'],
            legend: {{ textStyle: {{ color: '#333' }} }},
            xAxis: {{
                axisLabel: {{ color: '#666' }},
                axisLine: {{ lineStyle: {{ color: '#999' }} }},
                splitLine: {{ lineStyle: {{ color: '#eee' }} }}
            }},
            yAxis: {{
                axisLabel: {{ color: '#666' }},
                axisLine: {{ lineStyle: {{ color: '#999' }} }},
                splitLine: {{ lineStyle: {{ color: '#eee' }} }}
            }},
            tooltip: {{ backgroundColor: '#ffffff', textStyle: {{ color: '#333' }} }}
        }});
        echarts.registerTheme('dark', {{
            backgroundColor: '#121212',
            textStyle: {{ color: '#dddddd' }},
            color: ['#7eb6ff','#ffb366','#ff7b84','#6cd4d2','#6edb8f','#ffe34d','#c69cd9','#ffb3bd','#b8977a','#d0d0cf'],
            legend: {{ textStyle: {{ color: '#cccccc' }} }},
            xAxis: {{
                axisLabel: {{ color: '#bbbbbb' }},
                axisLine: {{ lineStyle: {{ color: '#888888' }} }},
                splitLine: {{ lineStyle: {{ color: '#333333' }} }}
            }},
            yAxis: {{
                axisLabel: {{ color: '#bbbbbb' }},
                axisLine: {{ lineStyle: {{ color: '#888888' }} }},
                splitLine: {{ lineStyle: {{ color: '#333333' }} }}
            }},
            tooltip: {{ backgroundColor: '#1e1e1e', textStyle: {{ color: '#dddddd' }} }}
        }});
        // Number formatting with max decimals and scientific notation when appropriate
        function trimZeros(str) {{
            if (typeof str !== 'string') return str;
            if (str.indexOf('e') !== -1 || str.indexOf('E') !== -1) {{
                var parts = str.split(/[eE]/);
                var mant = parts[0];
                var exp = parts[1];
                if (mant.indexOf('.') !== -1) {{
                    mant = mant.replace(/\.0+$/,'').replace(/(\.[0-9]*[1-9])0+$/,'$1').replace(/\.$/, '');
                }}
                return mant + 'e' + exp;
            }} else {{
                return str.replace(/\.0+$/,'').replace(/(\.[0-9]*[1-9])0+$/,'$1').replace(/\.$/, '');
            }}
        }}
        function formatNumber(val) {{
            if (typeof val !== 'number' || !isFinite(val)) return String(val);
            if (MAX_DECIMALS < 0) return String(val);
            var abs = Math.abs(val);
            var useSci = (abs !== 0) && (abs >= 1e6 || abs < 1e-4);
            var s = useSci ? val.toExponential(MAX_DECIMALS) : val.toFixed(MAX_DECIMALS);
            return trimZeros(s);
        }}
        var myChart = echarts.init(document.getElementById('main'), THEME);
        myChart.setOption({{
            animation: ANIMATIONS,
            title: {{ text: '{}', top: 5 }},
            tooltip: {{ trigger: 'axis', axisPointer: {{ type: 'cross' }}, valueFormatter: formatNumber }},
            legend: {{ type: 'scroll', top: 30 }},
            grid: {{ left: '2%', right: '2%', bottom: '6%', containLabel: true }},
            toolbox: {{
                feature: {{
                    dataZoom: {{ yAxisIndex: 'none' }},
                    restore: {{}},
                    saveAsImage: {{}}
                }}
            }},
            xAxis: {{ type: '{}', splitLine: {{ show: false }}{} }},
            yAxis: {{ type: 'value', axisLine: {{ show: true }}, axisLabel: {{ formatter: formatNumber }}, min: {}, max: {} }},
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

// Apply sizes and y-axis autoscale immediately and on zoom
function applySymbolSizes(startPct, endPct) {{
            var pct = Math.max(0, Math.min(1, endPct - startPct));
            var opt = myChart.getOption();
            var series = opt.series || [];
            var selected = (opt.legend && opt.legend[0] && opt.legend[0].selected) ? opt.legend[0].selected : null;
            var xAxis = (opt.xAxis && opt.xAxis[0]) ? opt.xAxis[0] : {{}};
            var xType = xAxis.type || 'value';
            var newSeries = series.map(function (s) {{
                var n = (s.metaN != null) ? s.metaN : ((s.data && s.data.length) ? s.data.length : 1000);
                var size = computeSize(n, pct);
                // Toggle large mode based on visible points for better styling accuracy when zoomed-in
                var visibleN = Math.max(1, Math.round(n * pct));
                var useLarge = visibleN > 5000;
                return {{ symbolSize: size, large: useLarge }};
            }});

            var updates = {{ series: newSeries }};

            if (AUTOSCALE_Y) {{
                // Y-axis autoscale based on visible window (time/value axis only)
                var yMin = Number.POSITIVE_INFINITY, yMax = Number.NEGATIVE_INFINITY;
                if (xType === 'time' || xType === 'value') {{
                    var allXMin = Number.POSITIVE_INFINITY, allXMax = Number.NEGATIVE_INFINITY;
                    for (var i = 0; i < series.length; i++) {{
                        var s = series[i];
                        if (selected && selected.hasOwnProperty && selected.hasOwnProperty(s.name) && !selected[s.name]) continue;
                        if (typeof s.metaXMin === 'number') allXMin = Math.min(allXMin, s.metaXMin);
                        if (typeof s.metaXMax === 'number') allXMax = Math.max(allXMax, s.metaXMax);
                    }}
                    if (isFinite(allXMin) && isFinite(allXMax) && allXMax > allXMin) {{
                        var startVal = allXMin + startPct * (allXMax - allXMin);
                        var endVal = allXMin + endPct * (allXMax - allXMin);
                        if (startVal > endVal) {{ var tmp = startVal; startVal = endVal; endVal = tmp; }}

                        // Scan data with stride for performance
                        for (var i = 0; i < series.length; i++) {{
                            var s = series[i];
                            if (selected && selected.hasOwnProperty && selected.hasOwnProperty(s.name) && !selected[s.name]) continue;
                            var d = s.data || [];
                            var estimate = (typeof s.metaN === 'number' ? s.metaN : d.length) * Math.max(0, endPct - startPct);
                            var stride = 1;
                            if (estimate > 5000 && d.length > 5000) {{
                                stride = Math.max(1, Math.floor(estimate / 2000));
                            }}
                            for (var j = 0; j < d.length; j += stride) {{
                                var p = d[j];
                                var x = Array.isArray(p) ? p[0] : null;
                                var y = Array.isArray(p) ? p[1] : null;
                                if (typeof x === 'number' && typeof y === 'number') {{
                                    if (x >= startVal && x <= endVal) {{
                                        if (isFinite(y)) {{
                                            if (y < yMin) yMin = y;
                                            if (y > yMax) yMax = y;
                                        }}
                                    }}
                                }}
                            }}
                        }}
                    }}
                }}

                var yAxisUpdate = {{}};
                if (isFinite(yMin) && isFinite(yMax)) {{
                    var span = Math.abs(yMax - yMin);
                    var pad = (span === 0) ? 1.0 : (span * 0.10);
                    yAxisUpdate.min = yMin - pad;
                    yAxisUpdate.max = yMax + pad;
                }}
                updates.yAxis = [yAxisUpdate];
            }}

            myChart.setOption(updates, false, false);
        }}

// Initial apply for full view
        applySymbolSizes(0.0, 1.0);

        // Adapt symbol sizes to the current zoom window
myChart.on('dataZoom', function () {{
            var opt = myChart.getOption();
            var dzArr = opt.dataZoom || [];
            if (!dzArr.length) {{ return; }}
            var dz = dzArr[0];
            var start = (dz.start != null) ? dz.start : 0;
            var end = (dz.end != null) ? dz.end : 100;
            var startPct = Math.max(0, Math.min(1, start / 100));
            var endPct = Math.max(0, Math.min(1, end / 100));
applySymbolSizes(startPct, endPct);
        }});
        // Re-apply autoscale on legend visibility changes
myChart.on('legendselectchanged', function () {{
            var opt = myChart.getOption();
            var dzArr = opt.dataZoom || [];
            if (!dzArr.length) {{ applySymbolSizes(0.0, 1.0); return; }}
            var dz = dzArr[0];
            var start = (dz.start != null) ? dz.start : 0;
            var end = (dz.end != null) ? dz.end : 100;
            var startPct = Math.max(0, Math.min(1, start / 100));
            var endPct = Math.max(0, Math.min(1, end / 100));
applySymbolSizes(startPct, endPct);
        }});
        // Re-apply sizes after toolbox restore resets options
myChart.on('restore', function () {{
            setTimeout(function() {{ applySymbolSizes(0.0, 1.0); }}, 0);
        }});
    </script>
</body>
</html>"#, 
        plot_data.title,
        autoscale_js,
        animations_js,
        max_decimals_js,
        use_white_js,
        plot_data.title,
        x_axis_type,
        x_axis_label_extra,
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
        let mut data_points: Vec<[Value; 2]> = Vec::new();
        let mut x_min = f64::INFINITY;
        let mut x_max = f64::NEG_INFINITY;
        let mut y_min = f64::INFINITY;
        let mut y_max = f64::NEG_INFINITY;

        for (x_val, y_val) in x_series.iter().zip(y_series.iter()) {
            if !matches!(x_val, AnyValue::Null) && !matches!(y_val, AnyValue::Null) {
                // JSON values for rendering
                let x_json = any_value_to_json_value(x_val.clone());
                let y_json = any_value_to_json_value(y_val.clone());
                data_points.push([x_json, y_json]);

                // Numeric values for meta range calculations (only numeric/time)
                if let (Some(xn), Some(yn)) = (any_value_to_f64(x_val), any_value_to_f64(y_val)) {
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

        let data_json = serde_json::to_string(&data_points)?;
        let n_points = data_points.len();
        let x_min_str = if x_min.is_finite() { format!("{}", x_min) } else { "null".to_string() };
        let x_max_str = if x_max.is_finite() { format!("{}", x_max) } else { "null".to_string() };
        let y_min_str = if y_min.is_finite() { format!("{}", y_min) } else { "null".to_string() };
        let y_max_str = if y_max.is_finite() { format!("{}", y_max) } else { "null".to_string() };

        let series_obj = format!(
            r#"{{
                name: '{}',
                type: 'scatter',
                metaN: {},
                metaXMin: {},
                metaXMax: {},
                metaYMin: {},
                metaYMax: {},
                symbolSize: (function() {{
                    const n = {};
                    return function(/* value, params */) {{
                        // Larger for small n, smaller for large n; hard minimum of 4px, cap ~36px
                        return Math.max(4, Math.min(36, (14 - Math.log10(n + 1) * 3.5) * 2));
                    }}
                }})(),
                large: true,
                largeThreshold: 2000,
                data: {}
            }}"#,
            y_series.name(),
            n_points,
            x_min_str,
            x_max_str,
            y_min_str,
            y_max_str,
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
