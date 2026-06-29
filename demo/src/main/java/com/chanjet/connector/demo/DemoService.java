package com.chanjet.connector.demo;

import com.chanjet.connector.demo.model.ManufactureOrderMsg;
import com.chanjet.connector.sdk.GatewayClient;
import com.chanjet.connector.sdk.MessageDispatcher;
import jakarta.annotation.PostConstruct;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Service;

@Service
public class DemoService {

    private static final Logger log = LoggerFactory.getLogger(DemoService.class);

    @Value("${chanjet.app-key:test_app}")
    private String appKey;

    @Value("${chanjet.app-secret:test_secret}")
    private String appSecret;

    @Value("${chanjet.gateway-url:}")
    private String gatewayUrl;

    @PostConstruct
    public void init() {
        log.info("Starting Java SDK Demo - Full Feature Reference...");

        MessageDispatcher dispatcher = new MessageDispatcher();

        // =====================================================================
        // 1. 标准系统消息订阅 (语义化快捷方法)
        // =====================================================================

        // A. 应用票据消息 (每10分钟推送)
        dispatcher.onAppTicket(msg -> {
            log.info("[System] 收到 APP_TICKET: {}", msg.getBizContent().getAppTicket());
            return true;
        });

        // B. 企业临时授权码 (应用开通/授权后推送)
        dispatcher.onEntAuthCode(msg -> {
            log.info("[System] 收到企业授权码: 企业={}, 码={}", msg.getAppId(), msg.getBizContent().getTempAuthCode());
            return true;
        });

        // C. 订单支付成功消息
        dispatcher.onOrderStatus(msg -> {
            log.info("[System] 订单支付成功: 单号={}, 金额={}", 
                msg.getBizContent().getOrderNo(), msg.getBizContent().getDetail().getPayTotal());
            return true;
        });

        // D. 取消授权/取消开通
        dispatcher.onEntUnauth(msg -> {
            log.info("[System] 企业取消授权: 企业ID={}, 用户ID={}", msg.getBizContent().getOrgId(), msg.getBizContent().getUserId());
            return true;
        });

        dispatcher.onAppCancelOpen(msg -> {
            log.info("[System] 应用取消开通: appId={}", msg.getBizContent().getAppId());
            return true;
        });


        // =====================================================================
        // 2. “好系列”（好生意、好业财）业务消息订阅 (无损语义化方法)
        // =====================================================================

        // 处理销货单 (GoodsIssue)
        dispatcher.onAppNotice("GoodsIssue", (msg, content) -> {
            log.info("[好系列] 收到销货单: 账套={}, 单号={}, 操作人={}", 
                msg.getBookCode(), content.getCode(), content.getUserName());
            return true;
        });

        // 处理特定交易类型的商品变更 (boName + transType)
        dispatcher.onAppNotice("Goods", "01", (msg, content) -> {
            log.info("[好系列] 商品基础信息变更: 单号={}, DataID={}", content.getCode(), content.getDataId());
            return true;
        });


        // =====================================================================
        // 3. 其他非标准或自定义业务消息 (通用 register 方法)
        // =====================================================================

        dispatcher.register("manufactureOrderMsg", ManufactureOrderMsg.class, msg -> {
            log.info("[T+] 收到生产加工单: 单号={}", msg.getBizContent().getOrderCode());
            return true;
        });


        // =====================================================================
        // 4. 启动 SDK 客户端
        // =====================================================================
        GatewayClient client = GatewayClient.builder()
                .appKey(appKey)
                .appSecret(appSecret)
                .gatewayUrl(gatewayUrl)
                .build();

        client.useDispatcher(dispatcher);
        client.start();
        
        log.info("SDK Client is running...");
    }
}
