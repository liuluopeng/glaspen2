import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_en.dart';
import 'app_localizations_zh.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AppLocalizations
/// returned by `AppLocalizations.of(context)`.
///
/// Applications need to include `AppLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'l10n/app_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AppLocalizations.localizationsDelegates,
///   supportedLocales: AppLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the AppLocalizations.supportedLocales
/// property.
abstract class AppLocalizations {
  AppLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AppLocalizations? of(BuildContext context) {
    return Localizations.of<AppLocalizations>(context, AppLocalizations);
  }

  static const LocalizationsDelegate<AppLocalizations> delegate =
      _AppLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('en'),
    Locale('zh'),
  ];

  /// No description provided for @canvasIsEmptyGo.
  ///
  /// In zh, this message translates to:
  /// **'空画布,先去涂鸦吧'**
  String get canvasIsEmptyGo;

  /// No description provided for @backToOrigin.
  ///
  /// In zh, this message translates to:
  /// **'回到原点'**
  String get backToOrigin;

  /// No description provided for @centerContent.
  ///
  /// In zh, this message translates to:
  /// **'居中内容'**
  String get centerContent;

  /// No description provided for @newCanvas.
  ///
  /// In zh, this message translates to:
  /// **'新建画布'**
  String get newCanvas;

  /// No description provided for @refresh.
  ///
  /// In zh, this message translates to:
  /// **'刷新'**
  String get refresh;

  /// No description provided for @redBoxCurrentMain.
  ///
  /// In zh, this message translates to:
  /// **'红框 = 主屏当前视口 · 涂鸦后点刷新'**
  String get redBoxCurrentMain;

  /// No description provided for @cRed.
  ///
  /// In zh, this message translates to:
  /// **'红色'**
  String get cRed;

  /// No description provided for @cOrange.
  ///
  /// In zh, this message translates to:
  /// **'橙色'**
  String get cOrange;

  /// No description provided for @cYellow.
  ///
  /// In zh, this message translates to:
  /// **'黄色'**
  String get cYellow;

  /// No description provided for @cGreen.
  ///
  /// In zh, this message translates to:
  /// **'绿色'**
  String get cGreen;

  /// No description provided for @cCyan.
  ///
  /// In zh, this message translates to:
  /// **'青色'**
  String get cCyan;

  /// No description provided for @cBlue.
  ///
  /// In zh, this message translates to:
  /// **'蓝色'**
  String get cBlue;

  /// No description provided for @cPurple.
  ///
  /// In zh, this message translates to:
  /// **'紫色'**
  String get cPurple;

  /// No description provided for @cPink.
  ///
  /// In zh, this message translates to:
  /// **'粉色'**
  String get cPink;

  /// No description provided for @cWhite.
  ///
  /// In zh, this message translates to:
  /// **'白色'**
  String get cWhite;

  /// No description provided for @cBlack.
  ///
  /// In zh, this message translates to:
  /// **'黑色'**
  String get cBlack;

  /// No description provided for @wUltraFine.
  ///
  /// In zh, this message translates to:
  /// **'极细'**
  String get wUltraFine;

  /// No description provided for @wVeryFine.
  ///
  /// In zh, this message translates to:
  /// **'很细'**
  String get wVeryFine;

  /// No description provided for @wFine.
  ///
  /// In zh, this message translates to:
  /// **'细'**
  String get wFine;

  /// No description provided for @wMedium.
  ///
  /// In zh, this message translates to:
  /// **'中'**
  String get wMedium;

  /// No description provided for @wThick.
  ///
  /// In zh, this message translates to:
  /// **'粗'**
  String get wThick;

  /// No description provided for @wVeryThick.
  ///
  /// In zh, this message translates to:
  /// **'很粗'**
  String get wVeryThick;

  /// No description provided for @wExtraThick.
  ///
  /// In zh, this message translates to:
  /// **'超粗'**
  String get wExtraThick;

  /// No description provided for @wUltraThick.
  ///
  /// In zh, this message translates to:
  /// **'极粗'**
  String get wUltraThick;

  /// No description provided for @glaspen2.
  ///
  /// In zh, this message translates to:
  /// **'玻璃涂鸦'**
  String get glaspen2;

  /// No description provided for @updateNow.
  ///
  /// In zh, this message translates to:
  /// **'立即更新'**
  String get updateNow;

  /// No description provided for @settings.
  ///
  /// In zh, this message translates to:
  /// **'设置'**
  String get settings;

  /// No description provided for @notebook.
  ///
  /// In zh, this message translates to:
  /// **'活页本'**
  String get notebook;

  /// No description provided for @freeDoodle.
  ///
  /// In zh, this message translates to:
  /// **'自由涂鸦'**
  String get freeDoodle;

  /// No description provided for @hotkeysCtrlAlt.
  ///
  /// In zh, this message translates to:
  /// **'快捷键 (Ctrl+Alt+…)'**
  String get hotkeysCtrlAlt;

  /// No description provided for @hotkeys.
  ///
  /// In zh, this message translates to:
  /// **'快捷键 (⌘⌃…)'**
  String get hotkeys;

  /// No description provided for @pen.
  ///
  /// In zh, this message translates to:
  /// **'笔'**
  String get pen;

  /// No description provided for @notebookLook.
  ///
  /// In zh, this message translates to:
  /// **'活页本外观'**
  String get notebookLook;

  /// No description provided for @style.
  ///
  /// In zh, this message translates to:
  /// **'风格'**
  String get style;

  /// No description provided for @skeuoGlass.
  ///
  /// In zh, this message translates to:
  /// **'拟物玻璃'**
  String get skeuoGlass;

  /// No description provided for @notePaper.
  ///
  /// In zh, this message translates to:
  /// **'笔记纸'**
  String get notePaper;

  /// No description provided for @truePassThroughExperimental.
  ///
  /// In zh, this message translates to:
  /// **'真穿透(实验)'**
  String get truePassThroughExperimental;

  /// No description provided for @cardsRevealTheDesktop.
  ///
  /// In zh, this message translates to:
  /// **'卡片透出面板后的桌面'**
  String get cardsRevealTheDesktop;

  /// No description provided for @clearGlassModeNeeds.
  ///
  /// In zh, this message translates to:
  /// **'清玻璃形态: 需拟物玻璃; 关闭恢复模糊+底图'**
  String get clearGlassModeNeeds;

  /// No description provided for @pageThumbnails.
  ///
  /// In zh, this message translates to:
  /// **'页面缩略图'**
  String get pageThumbnails;

  /// No description provided for @show10NearbyPages.
  ///
  /// In zh, this message translates to:
  /// **'显示附近 10 页'**
  String get show10NearbyPages;

  /// No description provided for @verticalMinimapOnThe.
  ///
  /// In zh, this message translates to:
  /// **'屏幕右缘竖向 minimap · 仅活页本(翻页)模式'**
  String get verticalMinimapOnThe;

  /// No description provided for @sharedCanvas.
  ///
  /// In zh, this message translates to:
  /// **'共享画布'**
  String get sharedCanvas;

  /// No description provided for @streamDoodlesLive.
  ///
  /// In zh, this message translates to:
  /// **'实时发送涂鸦'**
  String get streamDoodlesLive;

  /// No description provided for @pushedToThePeer.
  ///
  /// In zh, this message translates to:
  /// **'抬笔即推给对方应用当前打开的接收页;关闭即停'**
  String get pushedToThePeer;

  /// No description provided for @dataBackup.
  ///
  /// In zh, this message translates to:
  /// **'数据备份'**
  String get dataBackup;

  /// No description provided for @doodleIdentity.
  ///
  /// In zh, this message translates to:
  /// **'涂鸦身份'**
  String get doodleIdentity;

  /// No description provided for @about.
  ///
  /// In zh, this message translates to:
  /// **'关于'**
  String get about;

  /// No description provided for @random.
  ///
  /// In zh, this message translates to:
  /// **'随机'**
  String get random;

  /// No description provided for @noPagesYet.
  ///
  /// In zh, this message translates to:
  /// **'暂无页面'**
  String get noPagesYet;

  /// No description provided for @p1Pages.
  ///
  /// In zh, this message translates to:
  /// **'{p1} 页'**
  String p1Pages(Object p1);

  /// No description provided for @pastedOnPage2.
  ///
  /// In zh, this message translates to:
  /// **'在 -2 页粘贴的东西出现在 -1 页'**
  String get pastedOnPage2;

  /// No description provided for @pageIsEmptyNothing.
  ///
  /// In zh, this message translates to:
  /// **'页面为空, 没有可编辑的内容'**
  String get pageIsEmptyNothing;

  /// No description provided for @noStrokesInsideThe.
  ///
  /// In zh, this message translates to:
  /// **'圈内没有笔迹'**
  String get noStrokesInsideThe;

  /// No description provided for @p1StrokesSelectedBlue.
  ///
  /// In zh, this message translates to:
  /// **'已选 {p1} 笔(蓝色高亮) · 拖拽移动 / 右键复制或删除'**
  String p1StrokesSelectedBlue(Object p1);

  /// No description provided for @moveFailed.
  ///
  /// In zh, this message translates to:
  /// **'移动失败'**
  String get moveFailed;

  /// No description provided for @nothingToCopy.
  ///
  /// In zh, this message translates to:
  /// **'没有可复制的内容'**
  String get nothingToCopy;

  /// No description provided for @copiedP1Strokes.
  ///
  /// In zh, this message translates to:
  /// **'已复制 {p1} 笔'**
  String copiedP1Strokes(Object p1);

  /// No description provided for @clipboardIsEmpty.
  ///
  /// In zh, this message translates to:
  /// **'剪贴板是空的'**
  String get clipboardIsEmpty;

  /// No description provided for @pastedP1Strokes.
  ///
  /// In zh, this message translates to:
  /// **'已粘贴 {p1} 笔'**
  String pastedP1Strokes(Object p1);

  /// No description provided for @pasteFailed.
  ///
  /// In zh, this message translates to:
  /// **'粘贴失败'**
  String get pasteFailed;

  /// No description provided for @deletedP1Strokes.
  ///
  /// In zh, this message translates to:
  /// **'已删除 {p1} 笔'**
  String deletedP1Strokes(Object p1);

  /// No description provided for @deleteFailed.
  ///
  /// In zh, this message translates to:
  /// **'删除失败'**
  String get deleteFailed;

  /// No description provided for @nothingToUndo.
  ///
  /// In zh, this message translates to:
  /// **'没有可撤销的移动'**
  String get nothingToUndo;

  /// No description provided for @copySelection.
  ///
  /// In zh, this message translates to:
  /// **'复制所选'**
  String get copySelection;

  /// No description provided for @pasteHere.
  ///
  /// In zh, this message translates to:
  /// **'粘贴到这里'**
  String get pasteHere;

  /// No description provided for @deleteSelection.
  ///
  /// In zh, this message translates to:
  /// **'删除所选'**
  String get deleteSelection;

  /// No description provided for @notebooks.
  ///
  /// In zh, this message translates to:
  /// **'本子'**
  String get notebooks;

  /// No description provided for @pageP1P2Strokes.
  ///
  /// In zh, this message translates to:
  /// **'第 {p1} 页 · {p2} 笔已选'**
  String pageP1P2Strokes(Object p1, Object p2);

  /// No description provided for @copy.
  ///
  /// In zh, this message translates to:
  /// **'复制'**
  String get copy;

  /// No description provided for @undoMove.
  ///
  /// In zh, this message translates to:
  /// **'撤销移动'**
  String get undoMove;

  /// No description provided for @dragLassoSelectDrag.
  ///
  /// In zh, this message translates to:
  /// **'左键拖圈 = 选择 · 拖拽已选 = 移动 · 右键 = 复制/粘贴 · 点空白 = 取消选择'**
  String get dragLassoSelectDrag;

  /// No description provided for @noResponseOffBy.
  ///
  /// In zh, this message translates to:
  /// **'没反应/N-1 错觉'**
  String get noResponseOffBy;

  /// No description provided for @searchP1.
  ///
  /// In zh, this message translates to:
  /// **'搜索 “{p1}”'**
  String searchP1(Object p1);

  /// No description provided for @clearSearch.
  ///
  /// In zh, this message translates to:
  /// **'清除搜索'**
  String get clearSearch;

  /// No description provided for @searchTextInPages.
  ///
  /// In zh, this message translates to:
  /// **'搜索页内文字(OCR)'**
  String get searchTextInPages;

  /// No description provided for @ocrSearch.
  ///
  /// In zh, this message translates to:
  /// **'OCR 搜索'**
  String get ocrSearch;

  /// No description provided for @noMatchingPagesWas.
  ///
  /// In zh, this message translates to:
  /// **'没有匹配的页(该文字未被 OCR 识别?)'**
  String get noMatchingPagesWas;

  /// No description provided for @theShelfIsEmpty.
  ///
  /// In zh, this message translates to:
  /// **'本子是空的'**
  String get theShelfIsEmpty;

  /// No description provided for @otherwise.
  ///
  /// In zh, this message translates to:
  /// **', 否则'**
  String get otherwise;

  /// No description provided for @moveFailedReverted.
  ///
  /// In zh, this message translates to:
  /// **'移动失败, 已还原'**
  String get moveFailedReverted;

  /// No description provided for @multiSelect.
  ///
  /// In zh, this message translates to:
  /// **'多选'**
  String get multiSelect;

  /// No description provided for @p1PagesSelected.
  ///
  /// In zh, this message translates to:
  /// **'已选 {p1} 页'**
  String p1PagesSelected(Object p1);

  /// No description provided for @exportedP1PagesAs.
  ///
  /// In zh, this message translates to:
  /// **'已导出 {p1} 页 PDF 到桌面'**
  String exportedP1PagesAs(Object p1);

  /// No description provided for @exportFailed.
  ///
  /// In zh, this message translates to:
  /// **'导出失败'**
  String get exportFailed;

  /// No description provided for @exportPdf.
  ///
  /// In zh, this message translates to:
  /// **'导出 PDF'**
  String get exportPdf;

  /// No description provided for @deselectAll.
  ///
  /// In zh, this message translates to:
  /// **'取消全选'**
  String get deselectAll;

  /// No description provided for @selectAll.
  ///
  /// In zh, this message translates to:
  /// **'全选'**
  String get selectAll;

  /// No description provided for @deleting.
  ///
  /// In zh, this message translates to:
  /// **'删除中…'**
  String get deleting;

  /// No description provided for @deleteSelectedP1.
  ///
  /// In zh, this message translates to:
  /// **'删除所选({p1})'**
  String deleteSelectedP1(Object p1);

  /// No description provided for @cancel.
  ///
  /// In zh, this message translates to:
  /// **'取消'**
  String get cancel;

  /// No description provided for @batchDelete.
  ///
  /// In zh, this message translates to:
  /// **'批量删除'**
  String get batchDelete;

  /// No description provided for @deleteTheP1Selected.
  ///
  /// In zh, this message translates to:
  /// **'确定删除所选 {p1} 页及其所有笔迹吗?'**
  String deleteTheP1Selected(Object p1);

  /// No description provided for @deleteP1Pages.
  ///
  /// In zh, this message translates to:
  /// **'删除 {p1} 页'**
  String deleteP1Pages(Object p1);

  /// No description provided for @deletedP1Pages.
  ///
  /// In zh, this message translates to:
  /// **'已删除 {p1} 页'**
  String deletedP1Pages(Object p1);

  /// No description provided for @firstMostRecentlyActive.
  ///
  /// In zh, this message translates to:
  /// **'第一个(最近活跃)组'**
  String get firstMostRecentlyActive;

  /// No description provided for @pageP1.
  ///
  /// In zh, this message translates to:
  /// **'页面 {p1}'**
  String pageP1(Object p1);

  /// No description provided for @editContentLassoMove.
  ///
  /// In zh, this message translates to:
  /// **'编辑内容(圈选/移动/复制)'**
  String get editContentLassoMove;

  /// No description provided for @exportThisPageAs.
  ///
  /// In zh, this message translates to:
  /// **'导出此页 PNG'**
  String get exportThisPageAs;

  /// No description provided for @exportThisPageAs2.
  ///
  /// In zh, this message translates to:
  /// **'导出此页 SVG'**
  String get exportThisPageAs2;

  /// No description provided for @copyPageInfo.
  ///
  /// In zh, this message translates to:
  /// **'复制页信息'**
  String get copyPageInfo;

  /// No description provided for @deleteThisPage.
  ///
  /// In zh, this message translates to:
  /// **'删除此页面'**
  String get deleteThisPage;

  /// No description provided for @pngExportedToDesktop.
  ///
  /// In zh, this message translates to:
  /// **'已导出 PNG 到桌面'**
  String get pngExportedToDesktop;

  /// No description provided for @exportFailedEmptyPage.
  ///
  /// In zh, this message translates to:
  /// **'导出失败(页为空?)'**
  String get exportFailedEmptyPage;

  /// No description provided for @svgExportedToDesktop.
  ///
  /// In zh, this message translates to:
  /// **'已导出 SVG 到桌面'**
  String get svgExportedToDesktop;

  /// No description provided for @pageP1SizeP2.
  ///
  /// In zh, this message translates to:
  /// **'页 {p1}\\n尺寸 {p2}×{p3}\\n笔迹 {p4}\\n'**
  String pageP1SizeP2(Object p1, Object p2, Object p3, Object p4);

  /// No description provided for @createdP1.
  ///
  /// In zh, this message translates to:
  /// **'创建 {p1}'**
  String createdP1(Object p1);

  /// No description provided for @pageInfoCopied.
  ///
  /// In zh, this message translates to:
  /// **'页信息已复制'**
  String get pageInfoCopied;

  /// No description provided for @deletePage.
  ///
  /// In zh, this message translates to:
  /// **'删除页面'**
  String get deletePage;

  /// No description provided for @deletePageP1And.
  ///
  /// In zh, this message translates to:
  /// **'确定删除页面 {p1} 及其所有笔迹吗？'**
  String deletePageP1And(Object p1);

  /// No description provided for @delete.
  ///
  /// In zh, this message translates to:
  /// **'删除'**
  String get delete;

  /// No description provided for @deleteFailedP1.
  ///
  /// In zh, this message translates to:
  /// **'删除失败: {p1}'**
  String deleteFailedP1(Object p1);

  /// No description provided for @fireAndForget.
  ///
  /// In zh, this message translates to:
  /// **'发出去就不管'**
  String get fireAndForget;

  /// No description provided for @signedInHandwrittenMessages.
  ///
  /// In zh, this message translates to:
  /// **'登录成功,手写消息将携带身份'**
  String get signedInHandwrittenMessages;

  /// No description provided for @signInFailedP1.
  ///
  /// In zh, this message translates to:
  /// **'登录失败:{p1}'**
  String signInFailedP1(Object p1);

  /// No description provided for @enableHandwrittenMessagesSharing.
  ///
  /// In zh, this message translates to:
  /// **'启用手写消息与共享'**
  String get enableHandwrittenMessagesSharing;

  /// No description provided for @whenOff23.
  ///
  /// In zh, this message translates to:
  /// **'关闭时不占用 ⌘⌃2/⌘⌃3,涂鸦功能不受影响'**
  String get whenOff23;

  /// No description provided for @signingIn.
  ///
  /// In zh, this message translates to:
  /// **'正在登录…'**
  String get signingIn;

  /// No description provided for @signedIn.
  ///
  /// In zh, this message translates to:
  /// **'登录成功'**
  String get signedIn;

  /// No description provided for @accountConfigured23.
  ///
  /// In zh, this message translates to:
  /// **'已配置账号(⌘⌃2/⌘⌃3 手写消息将携带身份)'**
  String get accountConfigured23;

  /// No description provided for @notConfiguredMessagingStill.
  ///
  /// In zh, this message translates to:
  /// **'未配置:手写消息功能仍可用,只是发送时不带身份'**
  String get notConfiguredMessagingStill;

  /// No description provided for @serverAddress.
  ///
  /// In zh, this message translates to:
  /// **'服务地址'**
  String get serverAddress;

  /// No description provided for @eGHttps192.
  ///
  /// In zh, this message translates to:
  /// **'例如 https://192.168.31.58:23001'**
  String get eGHttps192;

  /// No description provided for @username.
  ///
  /// In zh, this message translates to:
  /// **'用户名'**
  String get username;

  /// No description provided for @password.
  ///
  /// In zh, this message translates to:
  /// **'密码'**
  String get password;

  /// No description provided for @savedLeaveBlankTo.
  ///
  /// In zh, this message translates to:
  /// **'已保存(留空保持不变)'**
  String get savedLeaveBlankTo;

  /// No description provided for @saveTestSignIn.
  ///
  /// In zh, this message translates to:
  /// **'保存并测试登录'**
  String get saveTestSignIn;

  /// No description provided for @authorAttributionFor2.
  ///
  /// In zh, this message translates to:
  /// **'用于 ⌘⌃2 手写草稿 / ⌘⌃3 直发的作者归属;涂鸦本身不依赖登录。'**
  String get authorAttributionFor2;

  /// No description provided for @frameRate.
  ///
  /// In zh, this message translates to:
  /// **'帧率'**
  String get frameRate;

  /// No description provided for @resolution.
  ///
  /// In zh, this message translates to:
  /// **'分辨率'**
  String get resolution;

  /// No description provided for @speed.
  ///
  /// In zh, this message translates to:
  /// **'速度'**
  String get speed;

  /// No description provided for @ending.
  ///
  /// In zh, this message translates to:
  /// **'结束时'**
  String get ending;

  /// No description provided for @stopOnLastFrame.
  ///
  /// In zh, this message translates to:
  /// **'停在最后'**
  String get stopOnLastFrame;

  /// No description provided for @hold1SThen.
  ///
  /// In zh, this message translates to:
  /// **'停1秒后循环'**
  String get hold1SThen;

  /// No description provided for @loopImmediately.
  ///
  /// In zh, this message translates to:
  /// **'立即循环'**
  String get loopImmediately;

  /// No description provided for @estimatedSizeP1.
  ///
  /// In zh, this message translates to:
  /// **'预估大小 {p1}'**
  String estimatedSizeP1(Object p1);

  /// No description provided for @quitApp.
  ///
  /// In zh, this message translates to:
  /// **'退出程序'**
  String get quitApp;

  /// No description provided for @exportImage.
  ///
  /// In zh, this message translates to:
  /// **'导出图片'**
  String get exportImage;

  /// No description provided for @previousPage.
  ///
  /// In zh, this message translates to:
  /// **'上一页'**
  String get previousPage;

  /// No description provided for @nextPage.
  ///
  /// In zh, this message translates to:
  /// **'下一页'**
  String get nextPage;

  /// No description provided for @undoLastStroke.
  ///
  /// In zh, this message translates to:
  /// **'撤销上一笔'**
  String get undoLastStroke;

  /// No description provided for @etherealCanvas.
  ///
  /// In zh, this message translates to:
  /// **'飘渺画布'**
  String get etherealCanvas;

  /// No description provided for @toggleDoodle.
  ///
  /// In zh, this message translates to:
  /// **'开关涂鸦'**
  String get toggleDoodle;

  /// No description provided for @blurBackground.
  ///
  /// In zh, this message translates to:
  /// **'模糊背景'**
  String get blurBackground;

  /// No description provided for @color.
  ///
  /// In zh, this message translates to:
  /// **'颜色'**
  String get color;

  /// No description provided for @width.
  ///
  /// In zh, this message translates to:
  /// **'粗细'**
  String get width;

  /// No description provided for @gifAnimationRTo.
  ///
  /// In zh, this message translates to:
  /// **'GIF 动画 (⌘⌃R 录制)'**
  String get gifAnimationRTo;

  /// No description provided for @gifAnimationHoldCtrl.
  ///
  /// In zh, this message translates to:
  /// **'GIF 动画 (Ctrl+Alt+R 按住录制)'**
  String get gifAnimationHoldCtrl;

  /// No description provided for @pageFlipEffect.
  ///
  /// In zh, this message translates to:
  /// **'翻页动效'**
  String get pageFlipEffect;

  /// No description provided for @systemAnimation.
  ///
  /// In zh, this message translates to:
  /// **'系统动效'**
  String get systemAnimation;

  /// No description provided for @timeTunnel.
  ///
  /// In zh, this message translates to:
  /// **'时光隧道'**
  String get timeTunnel;

  /// No description provided for @freeDoodleMode.
  ///
  /// In zh, this message translates to:
  /// **'自由涂鸦模式'**
  String get freeDoodleMode;

  /// No description provided for @showTheFreeDoodle.
  ///
  /// In zh, this message translates to:
  /// **'显示「自由涂鸦」画布:不翻页,镜头自由移动'**
  String get showTheFreeDoodle;

  /// No description provided for @pressureMonitor.
  ///
  /// In zh, this message translates to:
  /// **'压力监控'**
  String get pressureMonitor;

  /// No description provided for @showGrid.
  ///
  /// In zh, this message translates to:
  /// **'显示网格'**
  String get showGrid;

  /// No description provided for @helpsAlignWhileDoodling.
  ///
  /// In zh, this message translates to:
  /// **'涂鸦时辅助对齐'**
  String get helpsAlignWhileDoodling;

  /// No description provided for @gridSize.
  ///
  /// In zh, this message translates to:
  /// **'网格大小'**
  String get gridSize;

  /// No description provided for @columns.
  ///
  /// In zh, this message translates to:
  /// **'分栏'**
  String get columns;

  /// No description provided for @none.
  ///
  /// In zh, this message translates to:
  /// **'无'**
  String get none;

  /// No description provided for @twoColumnsSideBy.
  ///
  /// In zh, this message translates to:
  /// **'左右两栏'**
  String get twoColumnsSideBy;

  /// No description provided for @twoColumnsStacked.
  ///
  /// In zh, this message translates to:
  /// **'上下两栏'**
  String get twoColumnsStacked;

  /// No description provided for @grid3x3.
  ///
  /// In zh, this message translates to:
  /// **'九宫格'**
  String get grid3x3;

  /// No description provided for @gridFollowsDoodles.
  ///
  /// In zh, this message translates to:
  /// **'网格跟随涂鸦'**
  String get gridFollowsDoodles;

  /// No description provided for @whenOnTheGrid.
  ///
  /// In zh, this message translates to:
  /// **'开启后网格随涂鸦一起受 ⌘⌃X 控制'**
  String get whenOnTheGrid;

  /// No description provided for @glassFollowsDoodles.
  ///
  /// In zh, this message translates to:
  /// **'磨砂玻璃跟随涂鸦'**
  String get glassFollowsDoodles;

  /// No description provided for @whenOnTheGlass.
  ///
  /// In zh, this message translates to:
  /// **'开启后磨砂玻璃随涂鸦一起受 ⌘⌃X 控制；关闭则常驻'**
  String get whenOnTheGlass;

  /// No description provided for @inkOutline.
  ///
  /// In zh, this message translates to:
  /// **'笔迹描边'**
  String get inkOutline;

  /// No description provided for @dashedContrastOutlineVisible.
  ///
  /// In zh, this message translates to:
  /// **'黑白相间虚线描边，任意背景恒可见 · 已持久化'**
  String get dashedContrastOutlineVisible;

  /// No description provided for @softShadow.
  ///
  /// In zh, this message translates to:
  /// **'软阴影'**
  String get softShadow;

  /// No description provided for @softDarkShadowUnder.
  ///
  /// In zh, this message translates to:
  /// **'笔迹下方柔和黑影，同色背景保底可见 · 可与描边叠加'**
  String get softDarkShadowUnder;

  /// No description provided for @invertedInk.
  ///
  /// In zh, this message translates to:
  /// **'反色突出'**
  String get invertedInk;

  /// No description provided for @strokesTakeThePer.
  ///
  /// In zh, this message translates to:
  /// **'笔迹逐像素取背景反色 · 实验:视频背景持续追踪，建议关闭磨砂玻璃'**
  String get strokesTakeThePer;

  /// No description provided for @invertTrackingFpsCap.
  ///
  /// In zh, this message translates to:
  /// **'反色追踪帧率上限 · 静态背景不送帧零开销'**
  String get invertTrackingFpsCap;

  /// No description provided for @exportAllPagesStrokes.
  ///
  /// In zh, this message translates to:
  /// **'将全部页面的笔迹导出为 PDF 保存到桌面。'**
  String get exportAllPagesStrokes;

  /// No description provided for @exporting.
  ///
  /// In zh, this message translates to:
  /// **'导出中…'**
  String get exporting;

  /// No description provided for @exportAllPagesAs.
  ///
  /// In zh, this message translates to:
  /// **'导出全部页面为 PDF'**
  String get exportAllPagesAs;

  /// No description provided for @backupIsOneDb.
  ///
  /// In zh, this message translates to:
  /// **'备份是桌面上的单个 .db 文件(一致快照), 换机时可直接替换 glaspen2.db; '**
  String get backupIsOneDb;

  /// No description provided for @restoreMergesItNever.
  ///
  /// In zh, this message translates to:
  /// **'恢复是合并方式, 不会删除备份之后新画的内容。'**
  String get restoreMergesItNever;

  /// No description provided for @working.
  ///
  /// In zh, this message translates to:
  /// **'处理中…'**
  String get working;

  /// No description provided for @backUpAllData.
  ///
  /// In zh, this message translates to:
  /// **'备份全部数据'**
  String get backUpAllData;

  /// No description provided for @restoreFromLatestBackup.
  ///
  /// In zh, this message translates to:
  /// **'从最新备份恢复'**
  String get restoreFromLatestBackup;

  /// No description provided for @backedUpToP1.
  ///
  /// In zh, this message translates to:
  /// **'已备份到 {p1}'**
  String backedUpToP1(Object p1);

  /// No description provided for @backupFailedP1.
  ///
  /// In zh, this message translates to:
  /// **'备份失败: {p1}'**
  String backupFailedP1(Object p1);

  /// No description provided for @mergesTheNewestGlaspen2.
  ///
  /// In zh, this message translates to:
  /// **'将把桌面上最新的 glaspen2_backup_*.db 合并进当前数据: '**
  String get mergesTheNewestGlaspen2;

  /// No description provided for @pagesStrokesAlsoPresent.
  ///
  /// In zh, this message translates to:
  /// **'同名的页/笔迹以备份为准, 备份之后新画的内容会保留。'**
  String get pagesStrokesAlsoPresent;

  /// No description provided for @restore.
  ///
  /// In zh, this message translates to:
  /// **'恢复'**
  String get restore;

  /// No description provided for @pagesFromTheBackup.
  ///
  /// In zh, this message translates to:
  /// **'备份中的页将合并进当前库(不会删除备份之后\\n新画的内容)。继续吗?'**
  String get pagesFromTheBackup;

  /// No description provided for @restoredFromBackupP1.
  ///
  /// In zh, this message translates to:
  /// **'已从备份恢复: {p1}(重启后生效)'**
  String restoredFromBackupP1(Object p1);

  /// No description provided for @restoreFailedP1.
  ///
  /// In zh, this message translates to:
  /// **'恢复失败: {p1}'**
  String restoreFailedP1(Object p1);

  /// No description provided for @exportAllPages.
  ///
  /// In zh, this message translates to:
  /// **'导出全部页面'**
  String get exportAllPages;

  /// No description provided for @allPagesMergeInto.
  ///
  /// In zh, this message translates to:
  /// **'全部页将合成一个 PDF 保存到桌面。'**
  String get allPagesMergeInto;

  /// No description provided for @export.
  ///
  /// In zh, this message translates to:
  /// **'导出'**
  String get export;

  /// No description provided for @pdfSavedToDesktop.
  ///
  /// In zh, this message translates to:
  /// **'PDF 已保存到桌面'**
  String get pdfSavedToDesktop;

  /// No description provided for @pdfExportFailedP1.
  ///
  /// In zh, this message translates to:
  /// **'PDF 导出失败: {p1}'**
  String pdfExportFailedP1(Object p1);

  /// No description provided for @notConnectedToThe.
  ///
  /// In zh, this message translates to:
  /// **'未连接到主程序'**
  String get notConnectedToThe;

  /// No description provided for @updateToP1.
  ///
  /// In zh, this message translates to:
  /// **'更新到 {p1} ?'**
  String updateToP1(Object p1);

  /// No description provided for @downloadsTheNewPackage.
  ///
  /// In zh, this message translates to:
  /// **'将下载新版安装包、退出并自动重启玻璃涂鸦。\\n笔迹已实时存盘,不会丢失。'**
  String get downloadsTheNewPackage;

  /// No description provided for @whatSNew.
  ///
  /// In zh, this message translates to:
  /// **'更新内容:'**
  String get whatSNew;

  /// No description provided for @downloadUpdate.
  ///
  /// In zh, this message translates to:
  /// **'下载并更新'**
  String get downloadUpdate;

  /// No description provided for @downloadFailedP1.
  ///
  /// In zh, this message translates to:
  /// **'下载失败:{p1}'**
  String downloadFailedP1(Object p1);

  /// No description provided for @unpackFailed.
  ///
  /// In zh, this message translates to:
  /// **'解包失败'**
  String get unpackFailed;

  /// No description provided for @unpackFailedP1.
  ///
  /// In zh, this message translates to:
  /// **'解包失败:{p1}'**
  String unpackFailedP1(Object p1);

  /// No description provided for @quitForUpdateFailed.
  ///
  /// In zh, this message translates to:
  /// **'退出更新失败'**
  String get quitForUpdateFailed;

  /// No description provided for @quitForUpdateFailed2.
  ///
  /// In zh, this message translates to:
  /// **'退出更新失败:{p1}'**
  String quitForUpdateFailed2(Object p1);

  /// No description provided for @downloadingP1P2.
  ///
  /// In zh, this message translates to:
  /// **'下载中 {p1} / {p2}'**
  String downloadingP1P2(Object p1, Object p2);

  /// No description provided for @downloading.
  ///
  /// In zh, this message translates to:
  /// **'下载中…'**
  String get downloading;

  /// No description provided for @downloadedUnpackingVerifying.
  ///
  /// In zh, this message translates to:
  /// **'下载完成,正在解包校验…'**
  String get downloadedUnpackingVerifying;

  /// No description provided for @downloadedAndUnpackedTakes.
  ///
  /// In zh, this message translates to:
  /// **'已下载并解包完成。重启后生效。'**
  String get downloadedAndUnpackedTakes;

  /// No description provided for @restartToUpdate.
  ///
  /// In zh, this message translates to:
  /// **'立即重启更新'**
  String get restartToUpdate;

  /// No description provided for @quittingAndRestarting.
  ///
  /// In zh, this message translates to:
  /// **'正在退出并重启…'**
  String get quittingAndRestarting;

  /// No description provided for @updateFailedP1.
  ///
  /// In zh, this message translates to:
  /// **'更新失败:{p1}'**
  String updateFailedP1(Object p1);

  /// No description provided for @retry.
  ///
  /// In zh, this message translates to:
  /// **'重试'**
  String get retry;

  /// No description provided for @checking.
  ///
  /// In zh, this message translates to:
  /// **'正在检查…'**
  String get checking;

  /// No description provided for @newVersionFoundP1.
  ///
  /// In zh, this message translates to:
  /// **'发现新版本 {p1}'**
  String newVersionFoundP1(Object p1);

  /// No description provided for @openDownloadPage.
  ///
  /// In zh, this message translates to:
  /// **'打开下载页'**
  String get openDownloadPage;

  /// No description provided for @alreadyUpToDate.
  ///
  /// In zh, this message translates to:
  /// **'已是最新版本 (v{p1})'**
  String alreadyUpToDate(Object p1);

  /// No description provided for @checkFailedP1.
  ///
  /// In zh, this message translates to:
  /// **'检查失败:{p1}'**
  String checkFailedP1(Object p1);

  /// No description provided for @unknownError.
  ///
  /// In zh, this message translates to:
  /// **'未知错误'**
  String get unknownError;

  /// No description provided for @tapToQueryThe.
  ///
  /// In zh, this message translates to:
  /// **'点一下向 GitHub 查询最新正式版(需联网)'**
  String get tapToQueryThe;

  /// No description provided for @currentVersion.
  ///
  /// In zh, this message translates to:
  /// **'当前版本'**
  String get currentVersion;

  /// No description provided for @checkForUpdates.
  ///
  /// In zh, this message translates to:
  /// **'检查更新'**
  String get checkForUpdates;
}

class _AppLocalizationsDelegate
    extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  Future<AppLocalizations> load(Locale locale) {
    return SynchronousFuture<AppLocalizations>(lookupAppLocalizations(locale));
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['en', 'zh'].contains(locale.languageCode);

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

AppLocalizations lookupAppLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'en':
      return AppLocalizationsEn();
    case 'zh':
      return AppLocalizationsZh();
  }

  throw FlutterError(
    'AppLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
