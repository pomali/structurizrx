// A large, illustrative model of a Twitter-like social network, used as the
// interactive demo in the docs site (see site/build.sh). It is an educated
// approximation built from public engineering write-ups, not an accurate
// description of any real company's systems.
workspace "Twitter-like Social Network" "A large demo workspace: a microblogging platform with timelines, search, ads, trust & safety and data infrastructure." {

    !identifiers flat

    model {
        // ------------------------------------------------------------------
        // People
        // ------------------------------------------------------------------
        user = person "User" "Reads timelines, posts tweets, follows accounts, sends DMs."
        advertiser = person "Advertiser" "Buys promoted tweets and audience targeting."
        developer = person "Third-party Developer" "Builds apps and bots on the public API."
        moderator = person "Trust & Safety Agent" "Reviews reported content and enforces policy."
        sre = person "Site Reliability Engineer" "Operates the platform and responds to incidents."
        analyst = person "Data Scientist" "Runs experiments and analyses product metrics."

        // ------------------------------------------------------------------
        // External systems
        // ------------------------------------------------------------------
        apns = softwareSystem "Apple Push Notification service" "Delivers push notifications to iOS devices." { tags "External" }
        fcm = softwareSystem "Firebase Cloud Messaging" "Delivers push notifications to Android devices." { tags "External" }
        smsGateway = softwareSystem "SMS Gateway" "Sends SMS for login codes and alerts." { tags "External" }
        emailProvider = softwareSystem "Email Provider" "Sends transactional and digest email." { tags "External" }
        paymentProcessor = softwareSystem "Payment Processor" "Charges advertisers and subscribers." { tags "External" }
        cdnProvider = softwareSystem "CDN Provider" "Edge caching for images and video." { tags "External" }
        appStores = softwareSystem "App Stores" "Distribute the mobile apps and in-app subscriptions." { tags "External" }
        lawEnforcement = softwareSystem "Law Enforcement Portal" "Receives legal requests and emergency disclosures." { tags "External" }
        oauthProviders = softwareSystem "Identity Providers" "Sign in with Apple / Google." { tags "External" }

        // ------------------------------------------------------------------
        // The platform
        // ------------------------------------------------------------------
        twitter = softwareSystem "Social Network Platform" "Lets people publish short posts and follow each other." {

            group "Clients" {
                webApp = container "Web App" "Single-page web client." "TypeScript, React" {
                    tags "Browser"
                    properties { owner "client-team" }
                }
                iosApp = container "iOS App" "Native iPhone and iPad client." "Swift" {
                    tags "Mobile"
                    properties { owner "client-team" }
                }
                androidApp = container "Android App" "Native Android client." "Kotlin" {
                    tags "Mobile"
                    properties { owner "client-team" }
                }
            }

            group "Edge" {
                edgeLb = container "Edge Load Balancer" "Terminates TLS, rate-limits, routes by path." "Envoy" {
                    tags "Infrastructure"
                    perspective "security" "WAF rules, per-IP and per-token rate limits"
                    properties { owner "traffic-team" }
                }
                graphqlGateway = container "GraphQL Gateway" "Aggregates backend services into the first-party client API." "Scala, Finagle" {
                    properties { owner "api-platform-team" }
                    port gql "First-party GraphQL API" { protocol "HTTPS/GraphQL" direction in }
                }
                publicApi = container "Public API" "Versioned REST API for third-party developers." "Scala, Finagle" {
                    properties { owner "api-platform-team" }
                    perspective "security" "OAuth 2.0 scopes, app-level quotas"
                    port rest "v2 REST API" { protocol "HTTPS/JSON" direction in }
                }
                streamingApi = container "Streaming API" "Long-lived filtered streams of public tweets." "Java" {
                    properties { owner "api-platform-team" }
                }
                mediaUpload = container "Media Upload Service" "Chunked upload endpoint for images and video." "Go" {
                    properties { owner "media-team" }
                }
            }

            group "Identity" {
                authService = container "Auth Service" "Sessions, OAuth tokens, 2FA, login challenges." "Scala" {
                    properties { owner "identity-team" }
                    perspective "security" "Tokens signed with rotating keys; 2FA enforced for verified orgs"
                }
                userService = container "User Service" "Profiles, settings, verification state." "Scala" {
                    properties { owner "identity-team" }
                }
                accountDb = container "Account Store" "Users, credentials, devices." "MySQL (sharded)" {
                    tags "Database"
                    properties { owner "identity-team" }
                }
            }

            group "Social Graph" {
                graphService = container "Social Graph Service" "Follows, blocks, mutes, lists." "Scala" {
                    properties { owner "graph-team" }
                    perspective "performance" "p99 < 10ms for follow checks"
                }
                graphStore = container "Graph Store" "Adjacency lists, sharded by user id." "FlockDB-style on MySQL" {
                    tags "Database"
                    properties { owner "graph-team" }
                }
                whoToFollow = container "Who-to-Follow" "Account recommendations from graph walks." "Scala, GraphJet" {
                    properties { owner "recs-team" }
                }
            }

            group "Tweets" {
                tweetService = container "Tweet Service" "Create, read and delete tweets; enforces limits." "Scala" {
                    properties { owner "tweet-team" }
                    port write "Tweet write path" { protocol "Thrift" direction in }
                    port read "Tweet hydration" { protocol "Thrift" direction in }
                    port events "Tweet events" { protocol "Kafka" direction out description "TweetCreated / TweetDeleted" }
                }
                tweetStore = container "Tweet Store" "Durable tweet storage, time-ordered ids." "Manhattan (distributed KV)" {
                    tags "Database"
                    properties { owner "tweet-team" }
                }
                engagementService = container "Engagement Service" "Likes, retweets, replies, bookmarks and their counts." "Scala" {
                    properties { owner "tweet-team" }
                }
                countersStore = container "Counter Store" "Approximate engagement counters." "Redis Cluster" {
                    tags "Database"
                    properties { owner "tweet-team" }
                }
                idGenerator = container "ID Generator" "Roughly time-ordered 64-bit ids." "Snowflake" {
                    properties { owner "tweet-team" }
                }
            }

            group "Timelines" {
                fanoutService = container "Fanout Service" "Pushes new tweet ids into followers' home timelines." "Scala" {
                    properties { owner "timeline-team" }
                    perspective "performance" "Skips fanout for accounts with > 1M followers (pulled at read time)"

                    fanoutConsumer = component "Tweet Event Consumer" "Consumes TweetCreated events." "Kafka Streams"
                    followerLookup = component "Follower Lookup" "Pages through a tweeter's followers." "Scala"
                    celebrityPolicy = component "Celebrity Policy" "Decides push vs. pull for high-follower accounts." "Scala"
                    timelineWriter = component "Timeline Writer" "Appends tweet ids to cached timelines in batches." "Scala"

                    fanoutConsumer -> celebrityPolicy "asks push or pull"
                    fanoutConsumer -> followerLookup "requests followers"
                    followerLookup -> timelineWriter "streams follower batches to"
                }
                timelineService = container "Home Timeline Service" "Assembles the ranked home timeline." "Scala" {
                    properties { owner "timeline-team" }
                    perspective "performance" "p99 < 200ms end to end"

                    timelineApi = component "Timeline API" "Serves home timeline requests." "Finagle Thrift"
                    candidateSource = component "Candidate Sources" "In-network from cache, out-of-network from recs." "Scala"
                    heavyRanker = component "Heavy Ranker" "Scores candidates with the ranking model." "Scala, TensorFlow Serving client"
                    filters = component "Visibility Filters" "Drops blocked, muted, and policy-violating tweets." "Scala"
                    mixer = component "Timeline Mixer" "Interleaves ads, recommendations and tweets." "Scala"
                    hydrator = component "Hydrator" "Fetches tweet bodies, authors and counts." "Scala"

                    timelineApi -> candidateSource "gets candidates from"
                    candidateSource -> heavyRanker "sends candidates to"
                    heavyRanker -> filters "passes ranked list to"
                    filters -> mixer "passes filtered list to"
                    mixer -> hydrator "hydrates page with"
                }
                timelineCache = container "Timeline Cache" "Per-user lists of recent tweet ids." "Redis Cluster" {
                    tags "Database"
                    properties { owner "timeline-team" }
                }
                userTimelineService = container "Profile Timeline Service" "Tweets and replies by a single account." "Scala" {
                    properties { owner "timeline-team" }
                }
            }

            group "Discovery" {
                searchIngester = container "Search Ingester" "Tokenises tweets and feeds realtime indexes." "Java" {
                    properties { owner "search-team" }
                }
                searchIndex = container "Search Index" "Realtime and archive inverted indexes." "Earlybird (Lucene)" {
                    tags "Database"
                    properties { owner "search-team" }
                }
                searchService = container "Search Service" "Query parsing, blending and ranking." "Java" {
                    properties { owner "search-team" }
                }
                trendsService = container "Trends Service" "Detects trending topics per region." "Scala, Heron" {
                    properties { owner "search-team" }
                }
                recsService = container "Recommendation Service" "Out-of-network tweets and topics." "Scala" {
                    properties { owner "recs-team" }
                }
                mlServing = container "Model Serving" "Online inference for ranking and safety models." "TensorFlow Serving" {
                    properties { owner "ml-platform-team" }
                }
                featureStore = container "Feature Store" "Online user and tweet features." "Manhattan" {
                    tags "Database"
                    properties { owner "ml-platform-team" }
                }
            }

            group "Messaging & Notifications" {
                dmService = container "Direct Messages Service" "1:1 and group conversations." "Scala" {
                    properties { owner "messaging-team" }
                    perspective "security" "Optional end-to-end encryption for 1:1 conversations"
                }
                dmStore = container "DM Store" "Conversation and message storage." "Manhattan" {
                    tags "Database"
                    properties { owner "messaging-team" }
                }
                notificationService = container "Notification Service" "Decides what to notify, when, and on which channel." "Scala" {
                    properties { owner "notifications-team" }
                }
                pushGateway = container "Push Gateway" "Delivers mobile push to APNs and FCM." "Go" {
                    properties { owner "notifications-team" }
                }
                emailService = container "Email Service" "Templated email and digests." "Python" {
                    properties { owner "notifications-team" }
                }
            }

            group "Media" {
                mediaService = container "Media Service" "Transcodes video, resizes images, extracts thumbnails." "Go, FFmpeg" {
                    properties { owner "media-team" }
                }
                blobStore = container "Blob Store" "Original and derived media files." "Object storage" {
                    tags "Database"
                    properties { owner "media-team" }
                }
            }

            group "Ads" {
                adsManager = container "Ads Manager" "Campaign management UI for advertisers." "TypeScript, React" {
                    tags "Browser"
                    properties { owner "ads-team" }
                }
                adsApi = container "Ads API" "Campaigns, budgets, creatives, targeting." "Scala" {
                    properties { owner "ads-team" }
                }
                adServer = container "Ad Server" "Real-time auction and ad selection." "Scala" {
                    properties { owner "ads-team" }
                    perspective "performance" "Auction must complete in < 30ms"
                }
                adsDb = container "Ads Database" "Campaigns and budgets." "MySQL" {
                    tags "Database"
                    properties { owner "ads-team" }
                }
                billingService = container "Billing Service" "Invoices advertisers and subscription users." "Java" {
                    properties { owner "revenue-team" }
                    perspective "security" "PCI scope limited to the payment processor's tokenised cards"
                }
            }

            group "Trust & Safety" {
                safetyService = container "Safety Service" "Classifies tweets and accounts for spam and abuse." "Scala" {
                    properties { owner "trust-safety-team" }
                }
                reportService = container "Report Service" "Intake for user reports and appeals." "Scala" {
                    properties { owner "trust-safety-team" }
                }
                moderationConsole = container "Moderation Console" "Review queues and enforcement actions." "TypeScript, React" {
                    tags "Browser"
                    properties { owner "trust-safety-team" }
                }
                legalRequests = container "Legal Request Tool" "Tracks and fulfils lawful data requests." "Python" {
                    properties { owner "legal-eng-team" }
                }
            }

            group "Data Platform" {
                eventBus = container "Event Bus" "Durable, partitioned log of every platform event." "Kafka" {
                    tags "Queue"
                    properties { owner "data-platform-team" }
                }
                streamProcessing = container "Stream Processing" "Realtime aggregations and counters." "Heron / Flink" {
                    properties { owner "data-platform-team" }
                }
                dataLake = container "Data Lake" "Historical events and snapshots." "HDFS / Parquet" {
                    tags "Database"
                    properties { owner "data-platform-team" }
                }
                batchCompute = container "Batch Compute" "Offline jobs, model training, reporting." "Spark, Scalding" {
                    properties { owner "data-platform-team" }
                }
                experimentation = container "Experimentation Service" "Feature flags and A/B test assignment." "Scala" {
                    properties { owner "data-platform-team" }
                }
                analyticsUi = container "Analytics Notebooks" "Ad-hoc queries and dashboards." "Jupyter, Presto" {
                    tags "Browser"
                    properties { owner "data-platform-team" }
                }
            }

            group "Operations" {
                observability = container "Observability Stack" "Metrics, traces and logs for every service." "Prometheus, Zipkin, ELK" {
                    tags "Infrastructure"
                    properties { owner "sre-team" }
                }
                configService = container "Config & Service Discovery" "Dynamic config and service registry." "ZooKeeper" {
                    tags "Infrastructure"
                    properties { owner "sre-team" }
                }
            }
        }

        // ------------------------------------------------------------------
        // System level, for the landscape and context views. StructurizrX
        // does not derive implied relationships from the container-level
        // ones below, so these are stated once here.
        // ------------------------------------------------------------------
        user -> twitter "Reads, posts and messages using"
        advertiser -> twitter "Buys promoted tweets on"
        developer -> twitter "Builds apps against"
        moderator -> twitter "Enforces policy using"
        sre -> twitter "Operates"
        analyst -> twitter "Analyses and experiments on"
        twitter -> apns "Delivers iOS push via"
        twitter -> fcm "Delivers Android push via"
        twitter -> smsGateway "Sends login codes via"
        twitter -> emailProvider "Sends email via"
        twitter -> paymentProcessor "Charges cards via"
        twitter -> cdnProvider "Serves media through"
        twitter -> appStores "Verifies in-app purchases with"
        twitter -> lawEnforcement "Discloses data to"
        twitter -> oauthProviders "Federates sign-in with"

        // ------------------------------------------------------------------
        // People -> platform
        // ------------------------------------------------------------------
        user -> webApp "Reads and posts using"
        user -> iosApp "Reads and posts using"
        user -> androidApp "Reads and posts using"
        advertiser -> adsManager "Manages campaigns in"
        developer -> publicApi.rest "Builds apps against"
        developer -> streamingApi "Consumes filtered streams from"
        moderator -> moderationConsole "Reviews reports in"
        sre -> observability "Monitors the platform with"
        sre -> configService "Rolls out config changes with"
        analyst -> analyticsUi "Analyses metrics in"
        analyst -> experimentation "Configures experiments in"

        // Clients -> edge
        webApp -> edgeLb "Sends requests to" "HTTPS"
        iosApp -> edgeLb "Sends requests to" "HTTPS"
        androidApp -> edgeLb "Sends requests to" "HTTPS"
        adsManager -> edgeLb "Sends requests to" "HTTPS"
        moderationConsole -> edgeLb "Sends requests to" "HTTPS"
        webApp -> cdnProvider "Loads images and video from" "HTTPS"
        iosApp -> cdnProvider "Loads images and video from" "HTTPS"
        androidApp -> cdnProvider "Loads images and video from" "HTTPS"
        iosApp -> mediaUpload "Uploads media to" "HTTPS"
        androidApp -> mediaUpload "Uploads media to" "HTTPS"
        webApp -> mediaUpload "Uploads media to" "HTTPS"
        iosApp -> appStores "Purchases subscriptions through"
        androidApp -> appStores "Purchases subscriptions through"

        // Edge
        edgeLb -> graphqlGateway.gql "Routes first-party traffic to" "HTTP/2"
        edgeLb -> publicApi.rest "Routes third-party traffic to" "HTTP/2"
        edgeLb -> authService "Validates sessions with" "Thrift"
        graphqlGateway -> authService "Authorises requests with" "Thrift"
        publicApi -> authService "Checks OAuth tokens with" "Thrift"
        graphqlGateway -> tweetService.write "Posts tweets via" "Thrift"
        graphqlGateway -> tweetService.read "Hydrates tweets via" "Thrift"
        graphqlGateway -> timelineService "Fetches home timeline from" "Thrift"
        graphqlGateway -> userTimelineService "Fetches profile timelines from" "Thrift"
        graphqlGateway -> userService "Reads and updates profiles with" "Thrift"
        graphqlGateway -> graphService "Follows and blocks via" "Thrift"
        graphqlGateway -> engagementService "Likes and retweets via" "Thrift"
        graphqlGateway -> searchService "Searches via" "Thrift"
        graphqlGateway -> trendsService "Reads trends from" "Thrift"
        graphqlGateway -> dmService "Sends and reads messages via" "Thrift"
        graphqlGateway -> notificationService "Reads notification tab from" "Thrift"
        graphqlGateway -> whoToFollow "Gets account suggestions from" "Thrift"
        graphqlGateway -> reportService "Files reports with" "Thrift"
        graphqlGateway -> adsApi "Proxies Ads Manager calls to" "Thrift"
        graphqlGateway -> experimentation "Gets feature flags from" "Thrift"
        publicApi -> tweetService.write "Posts tweets via" "Thrift"
        publicApi -> tweetService.read "Hydrates tweets via" "Thrift"
        publicApi -> userService "Reads users from" "Thrift"
        publicApi -> graphService "Reads follows from" "Thrift"
        publicApi -> searchService "Searches via" "Thrift"
        streamingApi -> eventBus "Filters the public tweet stream from" "Kafka" { kind subscribe }
        mediaUpload -> blobStore "Stores originals in" "S3 API"
        mediaUpload -> eventBus "Publishes MediaUploaded to" "Kafka" { kind publish }

        // Identity
        authService -> accountDb "Reads credentials and sessions from" "SQL"
        authService -> smsGateway "Sends login codes via" "HTTPS"
        authService -> oauthProviders "Federates sign-in with" "OIDC"
        userService -> accountDb "Reads and writes profiles in" "SQL"
        userService -> eventBus "Publishes UserUpdated to" "Kafka" { kind publish }

        // Social graph
        graphService -> graphStore "Reads and writes edges in" "SQL"
        graphService -> eventBus "Publishes FollowCreated to" "Kafka" { kind publish }
        whoToFollow -> graphService "Walks the graph via" "Thrift"
        whoToFollow -> featureStore "Reads user features from" "Thrift"
        whoToFollow -> mlServing "Scores candidates with" "gRPC"

        // Tweets
        tweetService -> idGenerator "Allocates tweet ids from" "Thrift"
        tweetService -> tweetStore "Persists tweets in" "Thrift"
        tweetService -> safetyService "Checks new tweets with" "Thrift"
        tweetService -> mediaService "Attaches media via" "Thrift"
        tweetService.events -> eventBus "Publishes TweetCreated to" "Kafka" { kind publish }
        tweetService -> userService "Reads author state from" "Thrift"
        engagementService -> countersStore "Increments counters in" "Redis protocol"
        engagementService -> tweetService.read "Validates tweets via" "Thrift"
        engagementService -> eventBus "Publishes engagement events to" "Kafka" { kind publish }

        // Timelines
        fanoutConsumer -> eventBus "Consumes TweetCreated from" "Kafka" { kind subscribe }
        followerLookup -> graphService "Pages followers from" "Thrift"
        timelineWriter -> timelineCache "Appends tweet ids to" "Redis protocol"
        celebrityPolicy -> userService "Reads follower counts from" "Thrift"
        candidateSource -> timelineCache "Reads in-network ids from" "Redis protocol"
        candidateSource -> recsService "Gets out-of-network candidates from" "Thrift"
        heavyRanker -> mlServing "Scores candidates with" "gRPC"
        heavyRanker -> featureStore "Reads features from" "Thrift"
        filters -> graphService "Reads blocks and mutes from" "Thrift"
        filters -> safetyService "Reads visibility labels from" "Thrift"
        mixer -> adServer "Requests promoted tweets from" "Thrift"
        hydrator -> tweetService.read "Hydrates tweets via" "Thrift"
        hydrator -> engagementService "Reads counts from" "Thrift"
        hydrator -> userService "Reads authors from" "Thrift"
        timelineService -> experimentation "Reads ranking experiment buckets from" "Thrift"
        userTimelineService -> tweetStore "Scans an author's tweets in" "Thrift"
        userTimelineService -> tweetService.read "Hydrates tweets via" "Thrift"

        // Discovery
        searchIngester -> eventBus "Consumes tweet and engagement events from" "Kafka" { kind subscribe }
        searchIngester -> searchIndex "Indexes tweets into" "Thrift"
        searchService -> searchIndex "Queries" "Thrift"
        searchService -> tweetService.read "Hydrates results via" "Thrift"
        searchService -> mlServing "Ranks results with" "gRPC"
        trendsService -> eventBus "Counts hashtags and entities from" "Kafka" { kind subscribe }
        trendsService -> safetyService "Filters unsafe trends with" "Thrift"
        recsService -> graphService "Finds second-degree engagement via" "Thrift"
        recsService -> featureStore "Reads embeddings from" "Thrift"
        recsService -> mlServing "Scores candidates with" "gRPC"
        mlServing -> featureStore "Reads online features from" "Thrift"

        // Messaging and notifications
        dmService -> dmStore "Stores conversations in" "Thrift"
        dmService -> graphService "Checks who can message whom with" "Thrift"
        dmService -> safetyService "Scans links in messages with" "Thrift"
        dmService -> eventBus "Publishes MessageSent to" "Kafka" { kind publish }
        notificationService -> eventBus "Consumes follow, engagement, mention and message events from" "Kafka" { kind subscribe }
        notificationService -> userService "Reads notification settings from" "Thrift"
        notificationService -> mlServing "Predicts notification relevance with" "gRPC"
        notificationService -> pushGateway "Sends push via" "Thrift"
        notificationService -> emailService "Sends email via" "Thrift"
        pushGateway -> apns "Delivers iOS push via" "HTTP/2"
        pushGateway -> fcm "Delivers Android push via" "HTTPS"
        emailService -> emailProvider "Sends email via" "SMTP"

        // Media
        mediaService -> blobStore "Reads originals and writes renditions to" "S3 API"
        mediaService -> eventBus "Consumes MediaUploaded from" "Kafka" { kind subscribe }
        mediaService -> safetyService "Scans media with" "Thrift"
        cdnProvider -> blobStore "Pulls media from on cache miss" "HTTPS"

        // Ads
        adsApi -> adsDb "Reads and writes campaigns in" "SQL"
        adsApi -> billingService "Sets up payment methods with" "Thrift"
        adServer -> adsDb "Loads live campaigns from" "SQL"
        adServer -> featureStore "Reads targeting features from" "Thrift"
        adServer -> mlServing "Predicts click-through with" "gRPC"
        adServer -> eventBus "Publishes impressions to" "Kafka" { kind publish }
        billingService -> paymentProcessor "Charges cards via" "HTTPS"
        billingService -> appStores "Verifies in-app purchases with" "HTTPS"
        billingService -> streamProcessing "Reads spend aggregates from" "Thrift"
        billingService -> emailService "Sends invoices via" "Thrift"

        // Trust and safety
        safetyService -> mlServing "Classifies content with" "gRPC"
        safetyService -> eventBus "Publishes enforcement actions to" "Kafka" { kind publish }
        reportService -> eventBus "Publishes ReportFiled to" "Kafka" { kind publish }
        reportService -> safetyService "Prioritises reports with" "Thrift"
        moderationConsole -> reportService "Works review queues in" "HTTPS"
        moderationConsole -> safetyService "Applies labels and suspensions via" "HTTPS"
        moderationConsole -> legalRequests "Escalates legal cases to" "HTTPS"
        legalRequests -> lawEnforcement "Discloses data to" "HTTPS"
        legalRequests -> dataLake "Exports account data from" "Spark"

        // Data platform
        streamProcessing -> eventBus "Consumes all events from" "Kafka" { kind subscribe }
        streamProcessing -> countersStore "Reconciles counters in" "Redis protocol"
        streamProcessing -> featureStore "Writes realtime features to" "Thrift"
        eventBus -> dataLake "Archives events to" "Kafka Connect" { kind dataflow }
        batchCompute -> dataLake "Reads and writes datasets in" "HDFS"
        batchCompute -> featureStore "Publishes batch features to" "Thrift"
        batchCompute -> mlServing "Deploys trained models to" "Model registry" { kind deploy }
        analyticsUi -> dataLake "Queries" "Presto"
        experimentation -> eventBus "Logs assignments to" "Kafka" { kind publish }

        // Operations (every service reports telemetry; only the key ones are drawn)
        graphqlGateway -> observability "Emits traces to" "Zipkin" { kind async }
        timelineService -> observability "Emits metrics to" "Prometheus" { kind async }
        tweetService -> observability "Emits metrics to" "Prometheus" { kind async }
        graphqlGateway -> configService "Discovers backends via" "ZooKeeper"
        timelineService -> configService "Discovers backends via" "ZooKeeper"

        // ------------------------------------------------------------------
        // Deployment
        // ------------------------------------------------------------------
        production = deploymentEnvironment "Production" {
            deploymentNode "User device" "" "iOS / Android / Browser" {
                containerInstance webApp
                containerInstance iosApp
                containerInstance androidApp
            }
            deploymentNode "Primary data centre" "" "Bare metal, Mesos / Aurora" {
                lbNode = deploymentNode "Edge tier" "" "Linux" {
                    edgeInstance = containerInstance edgeLb
                }
                deploymentNode "API tier" "" "Mesos cluster" {
                    containerInstance graphqlGateway
                    containerInstance publicApi
                    containerInstance streamingApi
                    containerInstance mediaUpload
                }
                deploymentNode "Service tier" "" "Mesos cluster" {
                    containerInstance authService
                    containerInstance userService
                    containerInstance tweetService
                    containerInstance engagementService
                    containerInstance graphService
                    containerInstance fanoutService
                    containerInstance timelineService
                    containerInstance userTimelineService
                    containerInstance searchService
                    containerInstance dmService
                    containerInstance notificationService
                    containerInstance adServer
                    containerInstance safetyService
                }
                deploymentNode "Storage tier" "" "Dedicated hosts" {
                    containerInstance tweetStore
                    containerInstance accountDb
                    containerInstance graphStore
                    containerInstance timelineCache
                    containerInstance countersStore
                    containerInstance searchIndex
                }
                deploymentNode "Messaging tier" "" "Kafka brokers" {
                    containerInstance eventBus
                }
            }
            deploymentNode "Cloud region" "" "Public cloud" {
                deploymentNode "Analytics" "" "Managed Hadoop / Spark" {
                    containerInstance dataLake
                    containerInstance batchCompute
                }
                deploymentNode "Object storage" "" "S3-compatible" {
                    containerInstance blobStore
                }
            }
        }
    }

    views {
        systemLandscape "landscape" "System landscape" {
            include *
            autoLayout lr
        }
        systemContext twitter "context" "System context" {
            include *
            autoLayout lr
        }
        container twitter "containers" "All containers" {
            include *
            autoLayout lr
        }
        component fanoutService "fanout" "Fanout service components" {
            include *
            autoLayout lr
        }
        component timelineService "home-timeline" "Home timeline components" {
            include *
            autoLayout lr
        }
        // Impact analysis for the busiest services.
        auto focus tweetService { depth 1 }
        auto focus eventBus { direction in }
        auto focus graphService { direction in }
        // Cross-cutting slices.
        auto perspective "security"
        auto perspective "performance"
        auto slice relationship.kind==publish || relationship.kind==subscribe
        auto layer "Timelines"
        auto layer "Trust & Safety"

        container twitter "platform-core" "Core read and write paths" {
            include user webApp iosApp androidApp
            include edgeLb graphqlGateway publicApi mediaUpload
            include authService userService accountDb
            include graphService graphStore
            include tweetService tweetStore engagementService countersStore idGenerator
            include fanoutService timelineService timelineCache userTimelineService
            include searchIngester searchIndex searchService recsService
            include notificationService pushGateway dmService dmStore
            include mediaService blobStore eventBus
            autoLayout lr
        }

        dynamic twitter "post-tweet" "Posting a tweet and fanning it out to followers." {
            iosApp -> edgeLb "POST /graphql CreateTweet"
            edgeLb -> graphqlGateway "Routes request"
            graphqlGateway -> tweetService "CreateTweet"
            tweetService -> idGenerator "Allocate id"
            tweetService -> safetyService "Pre-publish safety check"
            tweetService -> tweetStore "Persist tweet"
            tweetService -> eventBus "Publish TweetCreated"
            fanoutService -> eventBus "Consume TweetCreated"
            fanoutService -> graphService "Page through followers"
            fanoutService -> timelineCache "Append tweet id to each follower's timeline"
            notificationService -> eventBus "Consume mentions"
            notificationService -> pushGateway "Notify mentioned users"
            searchIngester -> eventBus "Consume TweetCreated"
            searchIngester -> searchIndex "Index tweet"
            autoLayout lr
        }

        dynamic twitter "read-home-timeline" "Loading the ranked home timeline." {
            androidApp -> edgeLb "POST /graphql HomeTimeline"
            edgeLb -> graphqlGateway "Routes request"
            graphqlGateway -> timelineService "GetHomeTimeline"
            timelineService -> timelineCache "Read in-network tweet ids"
            timelineService -> recsService "Fetch out-of-network candidates"
            timelineService -> mlServing "Score candidates"
            timelineService -> graphService "Filter blocks and mutes"
            timelineService -> adServer "Request promoted tweets"
            timelineService -> tweetService "Hydrate tweets"
            timelineService -> engagementService "Hydrate counts"
            autoLayout lr
        }

        dynamic fanoutService "fanout-components" "Inside the fanout service." {
            fanoutConsumer -> eventBus "Consume TweetCreated"
            fanoutConsumer -> celebrityPolicy "Push or pull?"
            fanoutConsumer -> followerLookup "Request followers"
            followerLookup -> graphService "Page followers"
            followerLookup -> timelineWriter "Stream follower batches"
            timelineWriter -> timelineCache "Append tweet ids"
            autoLayout lr
        }

        deployment twitter "Production" "production-deployment" {
            include *
            autoLayout lr
        }

        styles {
            element "Person" {
                shape Person
                background #08427b
                color #ffffff
            }
            element "Software System" {
                background #1168bd
                color #ffffff
            }
            element "External" {
                background #8a8a8a
                color #ffffff
            }
            element "Container" {
                background #438dd5
                color #ffffff
            }
            element "Component" {
                background #85bbf0
                color #000000
            }
            element "Database" {
                shape Cylinder
            }
            element "Queue" {
                shape Pipe
            }
            element "Browser" {
                shape WebBrowser
            }
            element "Mobile" {
                shape MobileDevicePortrait
            }
            element "Infrastructure" {
                background #6c757d
            }
        }
    }
}
