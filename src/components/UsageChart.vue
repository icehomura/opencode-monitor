<template>
  <section class="chart-box">
    <h2>请求次数 / 输出·输入·缓存词元</h2>
    <div ref="chartRef" class="chart"></div>
  </section>
</template>

<script setup>
import { ref, watch, onMounted, onBeforeUnmount, nextTick } from 'vue'
import { fmtTokens } from '../utils/format'
import * as echarts from 'echarts/core'
import { BarChart, LineChart } from 'echarts/charts'
import {
  TitleComponent, TooltipComponent, GridComponent, LegendComponent, GraphicComponent,
} from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
import { themeColors, isLight, hexToRgba } from '../composables/useTheme'

echarts.use([
  BarChart, LineChart,
  TitleComponent, TooltipComponent, GridComponent, LegendComponent, GraphicComponent,
  CanvasRenderer,
])

const props = defineProps({
  points: { type: Array, default: () => [] },
  granularity: { type: String, default: 'day' },
  convertUnits: { type: Boolean, default: false },
})

const chartRef = ref(null)
let chart = null

// 固定配色：异常深红 / 正常灰 / 输出绿 / 输入蓝 / 缓存灰蓝
const COLOR = {
  normal: '#8b93a7',
  error: '#b3373f',
  output: '#2fbf71',
  input: '#4f8cff',
  cache: '#7f8aa3',
}

// 分钟粒度跨天时（本周 / 本月 / 全部）必须带日期，否则 HH:MM 无法区分是哪一天。
let multiDay = false

function labelOf(v) {
  const d = new Date(v)
  if (Number.isNaN(d.getTime())) return String(v)
  const p = (n) => String(n).padStart(2, '0')
  const md = `${p(d.getMonth() + 1)}-${p(d.getDate())}`
  switch (props.granularity) {
    case 'minute': {
      const hm = `${p(d.getHours())}:${p(d.getMinutes())}`
      return multiDay ? `${md} ${hm}` : hm
    }
    case 'hour': return `${md} ${p(d.getHours())}:00`
    default: return md
  }
}

/** 系列是否跨越一天以上（决定分钟刻度要不要带日期）。 */
function spansDays() {
  const first = Number(props.points[0]?.date)
  const last = Number(props.points[props.points.length - 1]?.date)
  return Number.isFinite(first) && Number.isFinite(last) && last - first > 86_400_000
}

function axis() {
  const tc = themeColors.value
  return {
    axisLine: { lineStyle: { color: tc.axis } },
    axisLabel: { color: tc.label, fontSize: 11 },
    splitLine: { lineStyle: { color: tc.split } },
  }
}

function tooltipStyle() {
  const light = isLight()
  const tc = themeColors.value
  return {
    backgroundColor: light ? 'rgba(255,255,255,.96)' : 'rgba(28,36,54,.96)',
    borderColor: light ? '#d7dee9' : '#263049',
    textStyle: { color: light ? '#2b3550' : '#dbe3f0', fontSize: 12 },
    axisPointer: {
      type: 'shadow',
      shadowStyle: { color: hexToRgba(tc.blue, 0.08) },
      lineStyle: { color: tc.axis },
    },
  }
}

function grad(c, top = 0.30) {
  return {
    color: {
      type: 'linear', x: 0, y: 0, x2: 0, y2: 1,
      colorStops: [
        { offset: 0, color: hexToRgba(c, top) },
        { offset: 1, color: hexToRgba(c, 0) },
      ],
    },
  }
}

// 柱子做成实色（不用渐变）

function renderChart() {
  if (!chart) return
  const tc = themeColors.value
  // 缓存线按主题取更低调的灰，避免抢眼
  const light = isLight()
  // 背景柱（正常请求）：很淡的中性灰，实色无渐变，不抢眼
  const barNormal = light ? 'rgba(120,132,152,0.28)' : 'rgba(150,162,184,0.20)'
  // 缓存线：偏蓝的板岩灰，和背景柱的中性灰拉开区别
  const cacheColor = light ? '#7d88a8' : '#7c88ad'
  multiDay = props.granularity === 'minute' && spansDays()
  const labels = props.points.map((p) => labelOf(p.date))
  const normal = props.points.map((p) => Math.max(0, Number(p.requests || 0) - Number(p.error_requests || 0)))
  const errors = props.points.map((p) => Number(p.error_requests || 0))
  const output = props.points.map((p) => Number(p.output_tokens || 0))
  const input = props.points.map((p) => Number(p.input_tokens || 0))
  const cache = props.points.map((p) => Number(p.cache_read_tokens || 0))

  // 轴名与数字列左对齐（数字左边缘 ≈ 轴位置 + 8）
  const GRID_RIGHT = 210, GRID_TOP = 48
  const W = chart.getWidth() || chartRef.value?.clientWidth || 1000
  const nameLeft = (offset) => (W - GRID_RIGHT + offset) + 8
  const axisNames = [
    { text: '输出', color: COLOR.output, offset: 0 },
    { text: '输入', color: COLOR.input, offset: 70 },
    { text: '缓存', color: cacheColor, offset: 140 },
  ]
  const graphic = axisNames.map((n) => ({
    type: 'text',
    left: Math.round(nameLeft(n.offset)),
    top: GRID_TOP - 26,
    style: { text: n.text, fill: n.color, font: '11px "Microsoft YaHei", sans-serif' },
    silent: true,
  }))

  chart.setOption(
    {
      backgroundColor: 'transparent',
      tooltip: {
        trigger: 'axis',
        ...tooltipStyle(),
        formatter(params) {
          if (!params || !params.length) return ''
          let s = `<div style="font-size:12px;margin-bottom:4px">${params[0].axisValue}</div>`
          for (const p of params) {
            const isReq = p.seriesName === '正常请求' || p.seriesName === '异常请求'
            const val = isReq
              ? (Math.round(p.value) || 0).toLocaleString() + ' 次'
              : fmtTokens(Math.round(p.value) || 0, props.convertUnits)
            s += `<div style="display:flex;align-items:center;gap:6px;margin:2px 0">`
            s += `<span style="display:inline-block;width:8px;height:8px;border-radius:50%;background:${p.color}"></span>`
            s += `<span>${p.seriesName}：</span><b>${val}</b></div>`
          }
          return s
        },
      },
      legend: {
        data: ['正常请求', '异常请求', '输出', '输入', '缓存'],
        top: 0,
        textStyle: { color: tc.label, fontSize: 12 },
      },
      grid: { left: 56, right: 210, top: 48, bottom: 40 },
      graphic,
      xAxis: { type: 'category', data: labels, ...axis(), boundaryGap: true },
      yAxis: [
        {
          type: 'value', name: '请求次数', position: 'left',
          nameTextStyle: { color: tc.label, align: 'left' },
          ...axis(),
          axisLabel: { ...axis().axisLabel, formatter: (v) => fmtTokens(v, props.convertUnits) },
          splitLine: { lineStyle: { color: tc.split } },
        },
        {
          type: 'value', position: 'right', offset: 0,
          ...axis(),
          axisLabel: { ...axis().axisLabel, color: COLOR.output, formatter: (v) => fmtTokens(v, props.convertUnits) },
          splitLine: { show: false },
        },
        {
          type: 'value', position: 'right', offset: 70,
          ...axis(),
          axisLabel: { ...axis().axisLabel, color: COLOR.input, formatter: (v) => fmtTokens(v, props.convertUnits) },
          splitLine: { show: false },
        },
        {
          type: 'value', position: 'right', offset: 140,
          ...axis(),
          axisLabel: { ...axis().axisLabel, color: cacheColor, formatter: (v) => fmtTokens(v, props.convertUnits) },
          splitLine: { show: false },
        },
      ],
      series: [
        {
          name: '异常请求', type: 'bar', stack: 'req', yAxisIndex: 0, data: errors,
          itemStyle: { color: COLOR.error, borderRadius: [0, 0, 0, 0] },
          barMaxWidth: 30,
        },
        {
          name: '正常请求', type: 'bar', stack: 'req', yAxisIndex: 0, data: normal,
          itemStyle: { color: barNormal, borderRadius: [3, 3, 0, 0] },
          barMaxWidth: 30,
        },
        {
          name: '输出', type: 'line', yAxisIndex: 1, data: output,
          smooth: true, symbol: 'circle', symbolSize: 4,
          lineStyle: { color: COLOR.output, width: 2.4 }, itemStyle: { color: COLOR.output },
          areaStyle: grad(COLOR.output),
        },
        {
          name: '输入', type: 'line', yAxisIndex: 2, data: input,
          smooth: true, symbol: 'circle', symbolSize: 4,
          lineStyle: { color: COLOR.input, width: 2.4 }, itemStyle: { color: COLOR.input },
          areaStyle: grad(COLOR.input),
        },
        {
          name: '缓存', type: 'line', yAxisIndex: 3, data: cache,
          smooth: true, symbol: 'circle', symbolSize: 3,
          lineStyle: { color: cacheColor, width: 1.6 }, itemStyle: { color: cacheColor },
          areaStyle: grad(cacheColor, 0.10),
        },
      ],
    },
    { notMerge: true },
  )
}

watch(
  () =>
    `${props.points.map((p) => `${p.date}:${p.requests}:${p.error_requests}:${p.input_tokens}:${p.output_tokens}:${p.cache_read_tokens}`).join('|')}|${props.granularity}|${themeColors.value.blue}|${props.convertUnits}`,
  () => nextTick(() => renderChart()),
)

onMounted(async () => {
  await nextTick()
  chart = echarts.init(chartRef.value)
  renderChart()
  // 首次布局完成后重算一次（getWidth 首次可能不准）
  requestAnimationFrame(() => renderChart())
  window.addEventListener('resize', () => { chart?.resize(); renderChart() })
  new ResizeObserver(() => { chart?.resize(); renderChart() }).observe(chartRef.value)
})

onBeforeUnmount(() => {
  chart?.dispose()
  chart = null
})

defineExpose({ renderChart })
</script>

<style scoped>
.chart-box {
  background: var(--panel); border: 1px solid var(--border);
  border-radius: 10px; padding: 14px;
  height: 100%;
  display: flex; flex-direction: column;
  overflow: hidden;
}
.chart-box h2 { font-size: 13px; color: var(--muted); font-weight: 500; margin-bottom: 8px; flex-shrink: 0; }
.chart { width: 100%; flex: 1; min-height: 0; }
</style>
