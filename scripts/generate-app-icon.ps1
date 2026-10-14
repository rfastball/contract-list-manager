# Authored vector geometry, rendered with Windows WPF; no extra image tools required.
# Regenerate: powershell -NoProfile -ExecutionPolicy Bypass -STA -File scripts/generate-app-icon.ps1
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName PresentationCore, WindowsBase
$iconDir = Join-Path $PSScriptRoot '../src/Pclm.App/Assets'
$previewDir = Join-Path $PSScriptRoot '../artifacts/icon'
New-Item -ItemType Directory -Force -Path $iconDir, $previewDir | Out-Null

$shapes = @(
    @{ Color = '#2f5fbf'; Path = 'M 6,2 L 19,2 28,11 28,28 Q 28,30 26,30 L 6,30 Q 4,30 4,28 L 4,4 Q 4,2 6,2 Z' },
    @{ Color = '#91abe4'; Path = 'M 19,2 L 19,9 Q 19,11 21,11 L 28,11 Z' },
    @{ Color = '#ffffff'; Path = 'M 9,11 L 15,11 M 9,16 L 22,16'; Width = 2 },
    @{ Color = '#ffffff'; Path = 'M 10,23 L 14,26 22,21'; Width = 2.5 }
)
$sizes = @(16, 20, 24, 32, 40, 48, 64, 128, 256)
$frames = foreach ($size in $sizes) {
    $visual = [System.Windows.Media.DrawingVisual]::new()
    $drawing = $visual.RenderOpen()
    $drawing.PushTransform([System.Windows.Media.ScaleTransform]::new($size / 32.0, $size / 32.0))
    foreach ($shape in $shapes) {
        $brush = [System.Windows.Media.BrushConverter]::new().ConvertFromString($shape.Color)
        $geometry = [System.Windows.Media.Geometry]::Parse($shape.Path)
        if ($shape.Width) {
            $pen = [System.Windows.Media.Pen]::new($brush, $shape.Width)
            $pen.StartLineCap = $pen.EndLineCap = [System.Windows.Media.PenLineCap]::Round
            $pen.LineJoin = [System.Windows.Media.PenLineJoin]::Round
            $drawing.DrawGeometry($null, $pen, $geometry)
        } else {
            $drawing.DrawGeometry($brush, $null, $geometry)
        }
    }
    $drawing.Pop()
    $drawing.Close()
    $bitmap = [System.Windows.Media.Imaging.RenderTargetBitmap]::new($size, $size, 96, 96, [System.Windows.Media.PixelFormats]::Pbgra32)
    $bitmap.Render($visual)
    $encoder = [System.Windows.Media.Imaging.PngBitmapEncoder]::new()
    $encoder.Frames.Add([System.Windows.Media.Imaging.BitmapFrame]::Create($bitmap))
    $stream = [System.IO.MemoryStream]::new()
    $encoder.Save($stream)
    $bytes = $stream.ToArray()
    $stream.Dispose()
    [System.IO.File]::WriteAllBytes((Join-Path $previewDir "app-$size.png"), $bytes)
    @{ Size = $size; Bytes = $bytes }
}

$file = [System.IO.File]::Create((Join-Path $iconDir 'app.ico'))
$writer = [System.IO.BinaryWriter]::new($file)
try {
    $writer.Write([uint16]0)
    $writer.Write([uint16]1)
    $writer.Write([uint16]$frames.Count)
    $offset = 6 + 16 * $frames.Count
    foreach ($frame in $frames) {
        $dimension = if ($frame.Size -eq 256) { 0 } else { $frame.Size }
        $writer.Write([byte]$dimension)
        $writer.Write([byte]$dimension)
        $writer.Write([uint16]0)
        $writer.Write([uint16]1)
        $writer.Write([uint16]32)
        $writer.Write([uint32]$frame.Bytes.Length)
        $writer.Write([uint32]$offset)
        $offset += $frame.Bytes.Length
    }
    foreach ($frame in $frames) { $writer.Write([byte[]]$frame.Bytes) }
} finally { $writer.Dispose() }
Write-Output "Generated app.ico ($($sizes -join ', ') px) and previews in artifacts/icon."
