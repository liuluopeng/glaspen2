// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Chinese (`zh`).
class AppLocalizationsZh extends AppLocalizations {
  AppLocalizationsZh([String locale = 'zh']) : super(locale);

  @override
  String get canvasIsEmptyGo => '空画布,先去涂鸦吧';

  @override
  String get backToOrigin => '回到原点';

  @override
  String get centerContent => '居中内容';

  @override
  String get newCanvas => '新建画布';

  @override
  String get refresh => '刷新';

  @override
  String get redBoxCurrentMain => '红框 = 主屏当前视口 · 涂鸦后点刷新';

  @override
  String get cRed => '红色';

  @override
  String get cOrange => '橙色';

  @override
  String get cYellow => '黄色';

  @override
  String get cGreen => '绿色';

  @override
  String get cCyan => '青色';

  @override
  String get cBlue => '蓝色';

  @override
  String get cPurple => '紫色';

  @override
  String get cPink => '粉色';

  @override
  String get cWhite => '白色';

  @override
  String get cBlack => '黑色';

  @override
  String get wUltraFine => '极细';

  @override
  String get wVeryFine => '很细';

  @override
  String get wFine => '细';

  @override
  String get wMedium => '中';

  @override
  String get wThick => '粗';

  @override
  String get wVeryThick => '很粗';

  @override
  String get wExtraThick => '超粗';

  @override
  String get wUltraThick => '极粗';

  @override
  String get glaspen2 => '玻璃涂鸦';

  @override
  String get updateNow => '立即更新';

  @override
  String get settings => '设置';

  @override
  String get notebook => '活页本';

  @override
  String get freeDoodle => '自由涂鸦';

  @override
  String get hotkeysCtrlAlt => '快捷键 (Ctrl+Alt+…)';

  @override
  String get hotkeys => '快捷键 (⌘⌃…)';

  @override
  String get pen => '笔';

  @override
  String get notebookLook => '活页本外观';

  @override
  String get style => '风格';

  @override
  String get skeuoGlass => '拟物玻璃';

  @override
  String get notePaper => '笔记纸';

  @override
  String get truePassThroughExperimental => '真穿透(实验)';

  @override
  String get cardsRevealTheDesktop => '卡片透出面板后的桌面';

  @override
  String get clearGlassModeNeeds => '清玻璃形态: 需拟物玻璃; 关闭恢复模糊+底图';

  @override
  String get pageThumbnails => '页面缩略图';

  @override
  String get show10NearbyPages => '显示附近 10 页';

  @override
  String get verticalMinimapOnThe => '屏幕右缘竖向 minimap · 仅活页本(翻页)模式';

  @override
  String get sharedCanvas => '共享画布';

  @override
  String get streamDoodlesLive => '实时发送涂鸦';

  @override
  String get pushedToThePeer => '抬笔即推给对方应用当前打开的接收页;关闭即停';

  @override
  String get dataBackup => '数据备份';

  @override
  String get doodleIdentity => '涂鸦身份';

  @override
  String get about => '关于';

  @override
  String get random => '随机';

  @override
  String get noPagesYet => '暂无页面';

  @override
  String p1Pages(Object p1) {
    return '$p1 页';
  }

  @override
  String get pastedOnPage2 => '在 -2 页粘贴的东西出现在 -1 页';

  @override
  String get pageIsEmptyNothing => '页面为空, 没有可编辑的内容';

  @override
  String get noStrokesInsideThe => '圈内没有笔迹';

  @override
  String p1StrokesSelectedBlue(Object p1) {
    return '已选 $p1 笔(蓝色高亮) · 拖拽移动 / 右键复制或删除';
  }

  @override
  String get moveFailed => '移动失败';

  @override
  String get nothingToCopy => '没有可复制的内容';

  @override
  String copiedP1Strokes(Object p1) {
    return '已复制 $p1 笔';
  }

  @override
  String get clipboardIsEmpty => '剪贴板是空的';

  @override
  String pastedP1Strokes(Object p1) {
    return '已粘贴 $p1 笔';
  }

  @override
  String get pasteFailed => '粘贴失败';

  @override
  String deletedP1Strokes(Object p1) {
    return '已删除 $p1 笔';
  }

  @override
  String get deleteFailed => '删除失败';

  @override
  String get nothingToUndo => '没有可撤销的移动';

  @override
  String get copySelection => '复制所选';

  @override
  String get pasteHere => '粘贴到这里';

  @override
  String get deleteSelection => '删除所选';

  @override
  String get notebooks => '本子';

  @override
  String pageP1P2Strokes(Object p1, Object p2) {
    return '第 $p1 页 · $p2 笔已选';
  }

  @override
  String get copy => '复制';

  @override
  String get undoMove => '撤销移动';

  @override
  String get dragLassoSelectDrag =>
      '左键拖圈 = 选择 · 拖拽已选 = 移动 · 右键 = 复制/粘贴 · 点空白 = 取消选择';

  @override
  String get noResponseOffBy => '没反应/N-1 错觉';

  @override
  String searchP1(Object p1) {
    return '搜索 “$p1”';
  }

  @override
  String get clearSearch => '清除搜索';

  @override
  String get searchTextInPages => '搜索页内文字(OCR)';

  @override
  String get ocrSearch => 'OCR 搜索';

  @override
  String get noMatchingPagesWas => '没有匹配的页(该文字未被 OCR 识别?)';

  @override
  String get theShelfIsEmpty => '本子是空的';

  @override
  String get otherwise => ', 否则';

  @override
  String get moveFailedReverted => '移动失败, 已还原';

  @override
  String get multiSelect => '多选';

  @override
  String p1PagesSelected(Object p1) {
    return '已选 $p1 页';
  }

  @override
  String exportedP1PagesAs(Object p1) {
    return '已导出 $p1 页 PDF 到桌面';
  }

  @override
  String get exportFailed => '导出失败';

  @override
  String get exportPdf => '导出 PDF';

  @override
  String get deselectAll => '取消全选';

  @override
  String get selectAll => '全选';

  @override
  String get deleting => '删除中…';

  @override
  String deleteSelectedP1(Object p1) {
    return '删除所选($p1)';
  }

  @override
  String get cancel => '取消';

  @override
  String get batchDelete => '批量删除';

  @override
  String deleteTheP1Selected(Object p1) {
    return '确定删除所选 $p1 页及其所有笔迹吗?';
  }

  @override
  String deleteP1Pages(Object p1) {
    return '删除 $p1 页';
  }

  @override
  String deletedP1Pages(Object p1) {
    return '已删除 $p1 页';
  }

  @override
  String get firstMostRecentlyActive => '第一个(最近活跃)组';

  @override
  String pageP1(Object p1) {
    return '页面 $p1';
  }

  @override
  String get editContentLassoMove => '编辑内容(圈选/移动/复制)';

  @override
  String get exportThisPageAs => '导出此页 PNG';

  @override
  String get exportThisPageAs2 => '导出此页 SVG';

  @override
  String get copyPageInfo => '复制页信息';

  @override
  String get deleteThisPage => '删除此页面';

  @override
  String get pngExportedToDesktop => '已导出 PNG 到桌面';

  @override
  String get exportFailedEmptyPage => '导出失败(页为空?)';

  @override
  String get svgExportedToDesktop => '已导出 SVG 到桌面';

  @override
  String pageP1SizeP2(Object p1, Object p2, Object p3, Object p4) {
    return '页 $p1\\n尺寸 $p2×$p3\\n笔迹 $p4\\n';
  }

  @override
  String createdP1(Object p1) {
    return '创建 $p1';
  }

  @override
  String get pageInfoCopied => '页信息已复制';

  @override
  String get deletePage => '删除页面';

  @override
  String deletePageP1And(Object p1) {
    return '确定删除页面 $p1 及其所有笔迹吗？';
  }

  @override
  String get delete => '删除';

  @override
  String deleteFailedP1(Object p1) {
    return '删除失败: $p1';
  }

  @override
  String get fireAndForget => '发出去就不管';

  @override
  String get signedInHandwrittenMessages => '登录成功,手写消息将携带身份';

  @override
  String signInFailedP1(Object p1) {
    return '登录失败:$p1';
  }

  @override
  String get enableHandwrittenMessagesSharing => '启用手写消息与共享';

  @override
  String get whenOff23 => '关闭时不占用 ⌘⌃2/⌘⌃3,涂鸦功能不受影响';

  @override
  String get signingIn => '正在登录…';

  @override
  String get signedIn => '登录成功';

  @override
  String get accountConfigured23 => '已配置账号(⌘⌃2/⌘⌃3 手写消息将携带身份)';

  @override
  String get notConfiguredMessagingStill => '未配置:手写消息功能仍可用,只是发送时不带身份';

  @override
  String get serverAddress => '服务地址';

  @override
  String get eGHttps192 => '例如 https://192.168.31.58:23001';

  @override
  String get username => '用户名';

  @override
  String get password => '密码';

  @override
  String get savedLeaveBlankTo => '已保存(留空保持不变)';

  @override
  String get saveTestSignIn => '保存并测试登录';

  @override
  String get authorAttributionFor2 => '用于 ⌘⌃2 手写草稿 / ⌘⌃3 直发的作者归属;涂鸦本身不依赖登录。';

  @override
  String get frameRate => '帧率';

  @override
  String get resolution => '分辨率';

  @override
  String get speed => '速度';

  @override
  String get ending => '结束时';

  @override
  String get stopOnLastFrame => '停在最后';

  @override
  String get hold1SThen => '停1秒后循环';

  @override
  String get loopImmediately => '立即循环';

  @override
  String estimatedSizeP1(Object p1) {
    return '预估大小 $p1';
  }

  @override
  String get quitApp => '退出程序';

  @override
  String get exportImage => '导出图片';

  @override
  String get previousPage => '上一页';

  @override
  String get nextPage => '下一页';

  @override
  String get undoLastStroke => '撤销上一笔';

  @override
  String get etherealCanvas => '飘渺画布';

  @override
  String get toggleDoodle => '开关涂鸦';

  @override
  String get blurBackground => '模糊背景';

  @override
  String get color => '颜色';

  @override
  String get width => '粗细';

  @override
  String get gifAnimationRTo => 'GIF 动画 (⌘⌃R 录制)';

  @override
  String get gifAnimationHoldCtrl => 'GIF 动画 (Ctrl+Alt+R 按住录制)';

  @override
  String get pageFlipEffect => '翻页动效';

  @override
  String get systemAnimation => '系统动效';

  @override
  String get timeTunnel => '时光隧道';

  @override
  String get freeDoodleMode => '自由涂鸦模式';

  @override
  String get showTheFreeDoodle => '显示「自由涂鸦」画布:不翻页,镜头自由移动';

  @override
  String get pressureMonitor => '压力监控';

  @override
  String get canvasLook => '画布外观';

  @override
  String get systemAndAssist => '系统与辅助';

  @override
  String get recordExport => '录制与导出';

  @override
  String get launchAtLogin => '开机自启';

  @override
  String get showGrid => '显示网格';

  @override
  String get bubbleTab => '气泡消息';

  @override
  String get showBubbleTabSub => '在 tab 栏显示手写消息回顾';

  @override
  String get bubblesEmpty => '还没有消息。去涂几笔 —— 停顿一下或换个地方写, 就是一条新消息';

  @override
  String get refreshBubbles => '刷新';

  @override
  String get bubbleAutoNewPage => '写完自动新建一页';

  @override
  String get bubbleAutoNewPageSub => '笔离开后落笔开新消息时, 自动翻到新的一页(需气泡 tab 开启)';

  @override
  String get bubbleCopySvg => '复制为 SVG 动画';

  @override
  String get bubbleSvgCopied => 'SVG 动画已复制到剪贴板';

  @override
  String get replayBubble => '重播';

  @override
  String get stopReplay => '停止';

  @override
  String get bubbleReplaySpeed => '重播速度';

  @override
  String get bubbleReplayHoldLast => '播完停在最后一帧';

  @override
  String get bubbleReplayHoldLastSub => '关闭时循环重播';

  @override
  String get bubbleReplayAuto => '打开时自动重播最新';

  @override
  String get bubbleReplayAutoSub => '进入气泡 tab 时重播最新一条';

  @override
  String get helpsAlignWhileDoodling => '涂鸦时辅助对齐';

  @override
  String get gridSize => '网格大小';

  @override
  String get columns => '分栏';

  @override
  String get none => '无';

  @override
  String get twoColumnsSideBy => '左右两栏';

  @override
  String get twoColumnsStacked => '上下两栏';

  @override
  String get grid3x3 => '九宫格';

  @override
  String get cornell => '康奈尔';

  @override
  String get gridFollowsDoodles => '网格跟随涂鸦';

  @override
  String get whenOnTheGrid => '开启后网格随涂鸦一起受 ⌘⌃X 控制';

  @override
  String get glassFollowsDoodles => '磨砂玻璃跟随涂鸦';

  @override
  String get whenOnTheGlass => '开启后磨砂玻璃随涂鸦一起受 ⌘⌃X 控制；关闭则常驻';

  @override
  String get inkOutline => '笔迹描边';

  @override
  String get dashedContrastOutlineVisible => '黑白相间虚线描边，任意背景恒可见 · 已持久化';

  @override
  String get softShadow => '软阴影';

  @override
  String get softDarkShadowUnder => '笔迹下方柔和黑影，同色背景保底可见 · 可与描边叠加';

  @override
  String get invertedInk => '反色突出';

  @override
  String get strokesTakeThePer => '笔迹逐像素取背景反色 · 实验:视频背景持续追踪，建议关闭磨砂玻璃';

  @override
  String get invertTrackingFpsCap => '反色追踪帧率上限 · 静态背景不送帧零开销';

  @override
  String get exportAllPagesStrokes => '将全部页面的笔迹导出为 PDF 保存到桌面。';

  @override
  String get exporting => '导出中…';

  @override
  String get exportAllPagesAs => '导出全部页面为 PDF';

  @override
  String get backupIsOneDb => '备份是桌面上的单个 .db 文件(一致快照), 换机时可直接替换 glaspen2.db; ';

  @override
  String get restoreMergesItNever => '恢复是合并方式, 不会删除备份之后新画的内容。';

  @override
  String get working => '处理中…';

  @override
  String get backUpAllData => '备份全部数据';

  @override
  String get restoreFromLatestBackup => '从最新备份恢复';

  @override
  String backedUpToP1(Object p1) {
    return '已备份到 $p1';
  }

  @override
  String backupFailedP1(Object p1) {
    return '备份失败: $p1';
  }

  @override
  String get mergesTheNewestGlaspen2 =>
      '将把桌面上最新的 glaspen2_backup_*.db 合并进当前数据: ';

  @override
  String get pagesStrokesAlsoPresent => '同名的页/笔迹以备份为准, 备份之后新画的内容会保留。';

  @override
  String get restore => '恢复';

  @override
  String get pagesFromTheBackup => '备份中的页将合并进当前库(不会删除备份之后\\n新画的内容)。继续吗?';

  @override
  String restoredFromBackupP1(Object p1) {
    return '已从备份恢复: $p1(重启后生效)';
  }

  @override
  String restoreFailedP1(Object p1) {
    return '恢复失败: $p1';
  }

  @override
  String get exportAllPages => '导出全部页面';

  @override
  String get allPagesMergeInto => '全部页将合成一个 PDF 保存到桌面。';

  @override
  String get export => '导出';

  @override
  String get pdfSavedToDesktop => 'PDF 已保存到桌面';

  @override
  String pdfExportFailedP1(Object p1) {
    return 'PDF 导出失败: $p1';
  }

  @override
  String get notConnectedToThe => '未连接到主程序';

  @override
  String updateToP1(Object p1) {
    return '更新到 $p1 ?';
  }

  @override
  String get downloadsTheNewPackage => '将下载新版安装包、退出并自动重启玻璃涂鸦。\\n笔迹已实时存盘,不会丢失。';

  @override
  String get whatSNew => '更新内容:';

  @override
  String get downloadUpdate => '下载并更新';

  @override
  String downloadFailedP1(Object p1) {
    return '下载失败:$p1';
  }

  @override
  String get unpackFailed => '解包失败';

  @override
  String unpackFailedP1(Object p1) {
    return '解包失败:$p1';
  }

  @override
  String get quitForUpdateFailed => '退出更新失败';

  @override
  String quitForUpdateFailed2(Object p1) {
    return '退出更新失败:$p1';
  }

  @override
  String downloadingP1P2(Object p1, Object p2) {
    return '下载中 $p1 / $p2';
  }

  @override
  String get downloading => '下载中…';

  @override
  String get downloadedUnpackingVerifying => '下载完成,正在解包校验…';

  @override
  String get downloadedAndUnpackedTakes => '已下载并解包完成。重启后生效。';

  @override
  String get restartToUpdate => '立即重启更新';

  @override
  String get quittingAndRestarting => '正在退出并重启…';

  @override
  String updateFailedP1(Object p1) {
    return '更新失败:$p1';
  }

  @override
  String get retry => '重试';

  @override
  String get checking => '正在检查…';

  @override
  String newVersionFoundP1(Object p1) {
    return '发现新版本 $p1';
  }

  @override
  String get openDownloadPage => '打开下载页';

  @override
  String alreadyUpToDate(Object p1) {
    return '已是最新版本 (v$p1)';
  }

  @override
  String checkFailedP1(Object p1) {
    return '检查失败:$p1';
  }

  @override
  String get unknownError => '未知错误';

  @override
  String get tapToQueryThe => '点一下向 GitHub 查询最新正式版(需联网)';

  @override
  String get currentVersion => '当前版本';

  @override
  String get checkForUpdates => '检查更新';

  @override
  String get options => '选项';

  @override
  String loadFailedP1(String p1) {
    return '加载失败: $p1';
  }

  @override
  String get language => '界面语言';

  @override
  String get chinese => '中文';

  @override
  String get english => 'English';
}
