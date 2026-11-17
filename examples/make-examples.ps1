# Builds the example documents as people's everyday files are: Excel saves
# the workbook (a fixed-width export with padded cells, formulas, a merged
# title, a row in gray, a row in red, one word in red inside a cell);
# make_docs.py writes two versions of a Word plan (pink highlights, a gray
# row, a dose in red). Run from this folder on Windows with Excel:
#   powershell -ExecutionPolicy Bypass -File make-examples.ps1
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
# Office colors are BGR integers.
function Bgr([int]$r, [int]$g, [int]$b) { $r + 256 * $g + 65536 * $b }

# ---------------------------------------------------------------- Excel
$xl = New-Object -ComObject Excel.Application
$xl.Visible = $false; $xl.DisplayAlerts = $false
try {
    $wb = $xl.Workbooks.Add()
    $ws = $wb.Worksheets.Item(1); $ws.Name = 'Inventory'
    $ws.Range('A1:F1').Merge()
    $ws.Range('A1').Value2 = 'Pet clinic - inventory, 4th quarter'
    $ws.Range('A1').Font.Bold = $true; $ws.Range('A1').Font.Size = 14
    $head = @('Code', 'Item', 'Unit', 'Qty', 'Unit price', 'Total')
    for ($i = 0; $i -lt $head.Count; $i++) { $ws.Cells.Item(3, $i + 1).Value2 = $head[$i] }
    $ws.Range('A3:F3').Font.Bold = $true
    $ws.Range('A3:F3').Interior.Color = Bgr 68 114 196
    $ws.Range('A3:F3').Font.Color = Bgr 255 255 255
    # A fixed-width export: every text cell padded with spaces to 20 characters.
    $pad = { param($t) $t.PadRight(20) }
    $rows = @(
        @('VAC-001', 'Rabies vaccine', 'dose', 120, 9.5),
        @('VAC-002', 'Parvovirus vaccine', 'dose', 80, 11.25),
        @('FOD-010', 'Dog food 15 kg', 'bag', 0, 38.0),
        @('FOD-011', 'Cat food 7 kg', 'bag', 25, 21.4),
        @('MED-100', 'Deworming tablets', 'box', 60, 6.8),
        @('MED-101', 'Flea collar', 'piece', 15, 12.0),
        @('OLD-900', 'Glass syringes', 'box', 4, 3.1)
    )
    $r = 4
    foreach ($row in $rows) {
        $ws.Cells.Item($r, 1).Value2 = (& $pad $row[0])
        $ws.Cells.Item($r, 2).Value2 = (& $pad $row[1])
        $ws.Cells.Item($r, 3).Value2 = (& $pad $row[2])
        $ws.Cells.Item($r, 4).Value2 = [double]$row[3]
        $ws.Cells.Item($r, 5).Value2 = [double]$row[4]
        $ws.Cells.Item($r, 6).Formula = "=D$r*E$r"
        $r++
    }
    $last = $r - 1
    $ws.Cells.Item($r, 5).Value2 = 'Total'
    $ws.Cells.Item($r, 6).Formula = "=SUM(F4:F$last)"
    $ws.Range("E$($r):F$r").Font.Bold = $true
    # Marks people use every day:
    $ws.Range('A6:F6').Font.Color = Bgr 255 0 0               # out of stock, in red
    $ws.Range('A10:F10').Interior.Color = Bgr 217 217 217     # discontinued, in gray
    $ws.Range('C8').Interior.Color = Bgr 255 242 204          # to check, light yellow
    $ws.Cells.Item($r + 2, 1).Value2 = 'Note: order only the items NOT in gray.'
    $ws.Cells.Item($r + 2, 1).Characters(18, 3).Font.Color = Bgr 255 0 0   # "NOT" in red
    $ws.Columns.Item('A:F').AutoFit() | Out-Null

    $sum = $wb.Worksheets.Add([Type]::Missing, $ws); $sum.Name = 'Summary'
    $sum.Range('A1').Value2 = 'Category'; $sum.Range('B1').Value2 = 'Total'
    $sum.Range('A1:B1').Font.Bold = $true
    $sum.Range('A2').Value2 = 'Vaccines'; $sum.Range('B2').Formula = "=SUMIF(Inventory!A4:A$last,""VAC*"",Inventory!F4:F$last)"
    $sum.Range('A3').Value2 = 'Food';     $sum.Range('B3').Formula = "=SUMIF(Inventory!A4:A$last,""FOD*"",Inventory!F4:F$last)"
    $sum.Range('A4').Value2 = 'Medicine'; $sum.Range('B4').Formula = "=SUMIF(Inventory!A4:A$last,""MED*"",Inventory!F4:F$last)"
    $wb.SaveAs((Join-Path $here 'clinic-inventory.xlsx'), 51)
    $wb.Close($false)
} finally { $xl.Quit(); [void][Runtime.InteropServices.Marshal]::ReleaseComObject($xl) }

# ----------------------------------------------------------------- Word
# The Word examples come from make_docs.py (Python standard library only), so
# they can be rebuilt on any system.
python (Join-Path $here 'make_docs.py')
Write-Host 'Listo: clinic-inventory.xlsx, vaccination-plan-v1.0.docx y vaccination-plan-v1.1.docx'
