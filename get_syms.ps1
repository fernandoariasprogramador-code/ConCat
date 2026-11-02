$log = [System.IO.File]::ReadAllLines('C:\Users\ferna\.gemini\antigravity-ide\brain\c9495c4d-3d40-446a-a5c0-1d265f5eaf4a\.system_generated\tasks\task-637.log')
$syms = @{}
foreach ($line in $log) {
    if ($line -match '(__std_\w+)') {
        $syms[$matches[1]] = $true
    }
}
$syms.Keys | Sort-Object
