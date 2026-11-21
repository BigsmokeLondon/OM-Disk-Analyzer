# Code signing

Signing the programs with a code-signing certificate shows **OM Industries Inc** as the
publisher and removes Windows' *"Unknown publisher"* / SmartScreen warnings.

## Getting a certificate
Windows only trusts certificates from a certificate authority. A self-made ("self-signed")
certificate is free but still shows the warning on every other computer.

| Option | Cost (approx.) | Notes |
|---|---|---|
| **SignPath Foundation** | free | For open-source projects only: the source must be public (e.g. on GitHub) and the project is reviewed first. |
| **Microsoft Trusted Signing** (Azure Artifact Signing) | ~US$10 / month | Trusted by SmartScreen straight away; needs an Azure account and identity check of the company. |
| **Standard (OV) certificate** from Certum, Sectigo, SSL.com, DigiCert ... | ~US$60–400 / year | Comes on a USB token or cloud key; Certum has a cheaper option for open-source developers. |

A brand-new certificate can still show a SmartScreen warning for the first downloads, until it
builds up reputation.

## Signing the files
Install the Windows SDK's *Signing Tools for Desktop Apps* (for `signtool.exe`), then in PowerShell
in the app folder:

    .\tools\package.ps1 -Pfx C:\path\om-industries.pfx       # build, sign and package everything
    .\tools\package.ps1 -Thumbprint 0123ABCD...               # certificate installed in Windows / on a token

or sign files already in `dist\`:

    .\tools\sign.ps1 -Pfx C:\path\om-industries.pfx

Signing uses SHA-256 and a public timestamp server, so signatures stay valid after the
certificate expires. Each signed file is verified afterwards.
