# Signing PressRoom

Until the installer is signed, Windows shows "Windows protected your PC" the first time someone opens
it (More info › Run anyway gets past it). A code-signing certificate makes that go away once the
certificate has built up a reputation with Microsoft SmartScreen.

`build-installer.ps1` signs `PressRoom.exe`, `PressRoom-cli.exe` and the MSI when the environment
variable `PRESSROOM_SIGN` holds a signing command with `{file}` where the file goes. Without it, the
build is unsigned, as before. `release.ps1` builds through `build-installer.ps1`, so set the variable
in the same window before releasing.

## Route 1: Azure Artifact Signing (formerly Trusted Signing)

About US$10 a month (Basic: 5,000 signatures). Individual developers in the USA and Canada can
sign up. The certificate carries your own name. Billing starts when the signing account is created,
even before identity validation passes.

1. In the Azure portal, create an Artifact Signing account, then an Identity validation (Individual),
   then a Certificate profile (Public Trust). Give your Azure user the "Artifact Signing Certificate
   Profile Signer" role on the account.
2. Install the Windows SDK's `signtool`, the Artifact Signing client tools (the `Azure.CodeSigning.Dlib.dll`
   package), and sign in with `az login`.
3. Write `metadata.json` beside the dlib:

   ```json
   { "Endpoint": "https://<region>.codesigning.azure.net", "CodeSigningAccountName": "<account>", "CertificateProfileName": "<profile>" }
   ```

4. Before building or releasing:

   ```powershell
   $env:PRESSROOM_SIGN = 'signtool sign /fd SHA256 /tr http://timestamp.acs.microsoft.com /td SHA256 /dlib C:\Tools\signing\Azure.CodeSigning.Dlib.dll /dmdf C:\Tools\signing\metadata.json "{file}"'
   ```

## Route 2: SignPath Foundation (free for open source)

Free for OSI-licensed public projects (PressRoom is MIT/Apache-2.0). The certificate is issued to
SignPath Foundation, so Windows shows "SignPath Foundation" as the publisher, and the README has to
say "Free code signing provided by SignPath.io, certificate by SignPath Foundation." Everyone on the
project needs two-factor sign-in on GitHub and SignPath. Signing happens on SignPath's servers from a
verifiable build, which means building the installer with GitHub Actions (they are turned off on the
repository today) instead of on this PC. Apply at https://signpath.org.

## Which

Route 1 keeps today's build-on-this-PC release and shows your name; it costs money every month.
Route 2 is free, but shows SignPath's name and moves the release build to GitHub Actions.
