# Channel Policy

Channels are the closed vocabulary `stable`, `beta`, `dev`. A stable installation follows stable only. Beta follows beta; development follows dev according to explicit administrator/user intent. The existing service wire currently exposes stable/beta; the release-authority contract reserves and validates `dev` without silently routing it through the stable/beta service path. Switching channel is an explicit action and never a metadata side effect. Package identity, signed metadata and trust key must all carry the same channel.
