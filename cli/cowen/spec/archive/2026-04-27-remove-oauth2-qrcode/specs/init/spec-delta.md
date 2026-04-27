# Specification Delta: Remove OAuth2 QR Code

## REMOVED Requirements

### Requirement: OAuth2 初始化二维码渲染
WHEN 系统生成 OAuth2 授权链接时,
系统 SHALL 渲染并显示对应的二维码。

## MODIFIED Requirements

### Requirement: OAuth2 授权引导
WHEN 进入 OAuth2 授权环节时,
系统 SHALL 告知用户必须在当前运行 CLI 的机器浏览器中完成授权。

#### Scenario: 引导文案更新
GIVEN 系统已生成授权 URL
WHEN 显示引导信息
THEN 系统 SHALL 明确提示 "请在本机浏览器中完成授权 (Please complete authorization in the local browser of this machine)"。
