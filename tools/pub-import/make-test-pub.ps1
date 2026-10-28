# Make a sample church bulletin in Microsoft Publisher (through its COM interface), with the kinds of
# objects real bulletins have: mixed fonts, sizes, bold/italic/colour, alignment, a picture, a
# filled shape, two linked text boxes, and a master-page footer. A test file for pub2pressroom.ps1.
param([Parameter(Mandatory)][string]$Out, [Parameter(Mandatory)][string]$Picture)

$app = New-Object -ComObject Publisher.Application
try {
    $doc = $app.NewDocument()
    $doc.PageSetup.PageWidth = 396   # 5.5 in
    $doc.PageSetup.PageHeight = 612  # 8.5 in
    [void]$doc.Pages.Add(3, 1)
    $rgb = { param($r, $g, $b) $r + 256 * $g + 65536 * $b }

    # Page 1: cover.
    $p1 = $doc.Pages.Item(1)
    $t = $p1.Shapes.AddTextbox(1, 36, 40, 324, 50)
    $t.TextFrame.TextRange.Text = "Grace Community Church"
    $t.TextFrame.TextRange.Font.Name = "Times New Roman"
    $t.TextFrame.TextRange.Font.Size = 24
    $t.TextFrame.TextRange.Font.Bold = -1
    $t.TextFrame.TextRange.Font.Color.RGB = (& $rgb 20 60 120)
    $t.TextFrame.TextRange.ParagraphFormat.Alignment = 2
    $pic = $p1.Shapes.AddPicture($Picture, 0, 98, 110, 200, 200)
    $box = $p1.Shapes.AddShape(1, 36, 330, 324, 6)
    $box.Fill.ForeColor.RGB = (& $rgb 200 150 40)
    $box.Line.Visible = 0
    $t2 = $p1.Shapes.AddTextbox(1, 36, 350, 324, 120)
    $t2.TextFrame.TextRange.Text = "Sunday Worship Service`rOctober 26, 2026 at 10:30 am`rWelcome! We are so glad you are here today."
    $t2.TextFrame.TextRange.Font.Name = "Calibri"
    $t2.TextFrame.TextRange.Font.Size = 12
    $t2.TextFrame.TextRange.ParagraphFormat.Alignment = 2
    $first = $t2.TextFrame.TextRange.Paragraphs(1, 1)
    $first.Font.Size = 18
    $first.Font.Bold = -1
    $t2.TextFrame.TextRange.Characters(53, 8).Font.Italic = -1

    # Pages 2-3: a long announcement running through two linked boxes.
    $p2 = $doc.Pages.Item(2)
    $h = $p2.Shapes.AddTextbox(1, 36, 36, 324, 36)
    $h.TextFrame.TextRange.Text = "Announcements"
    $h.TextFrame.TextRange.Font.Size = 18
    $h.TextFrame.TextRange.Font.Bold = -1
    $a = $p2.Shapes.AddTextbox(1, 36, 80, 324, 200)
    $b = $doc.Pages.Item(3).Shapes.AddTextbox(1, 36, 36, 324, 300)
    $a.TextFrame.NextLinkedTextFrame = $b.TextFrame
    $lines = @()
    foreach ($i in 1..18) { $lines += "Announcement $i - Bible study meets Wednesday at 7 pm in the fellowship hall. Everyone is welcome to join us." }
    $a.TextFrame.TextRange.Text = ($lines -join "`r")
    $a.TextFrame.TextRange.Font.Size = 11
    $a.TextFrame.TextRange.Paragraphs(3, 1).Font.Color.RGB = (& $rgb 180 30 30)

    # Page 4: prayer list with a bold label and a right-aligned contact line.
    $p4 = $doc.Pages.Item(4)
    $pr = $p4.Shapes.AddTextbox(1, 36, 36, 324, 200)
    $pr.TextFrame.TextRange.Text = "Prayer Requests`rThe Smith family`rOur missionaries overseas`rThose who are sick or grieving"
    $pr.TextFrame.TextRange.Font.Size = 12
    $pr.TextFrame.TextRange.Paragraphs(1, 1).Font.Bold = -1
    $c = $p4.Shapes.AddTextbox(1, 36, 520, 324, 40)
    $c.TextFrame.TextRange.Text = "Pastor John Doe · (555) 010-0199"
    $c.TextFrame.TextRange.ParagraphFormat.Alignment = 3

    # Master page footer: on every page.
    $m = $doc.MasterPages.Item(1)
    $f = $m.Shapes.AddTextbox(1, 36, 580, 324, 20)
    $f.TextFrame.TextRange.Text = "Grace Community Church"
    $f.TextFrame.TextRange.Font.Size = 8
    $f.TextFrame.TextRange.ParagraphFormat.Alignment = 2

    $doc.SaveAs($Out)
    $doc.Close()
    "saved $Out"
}
finally {
    $app.Quit()
    [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
}
