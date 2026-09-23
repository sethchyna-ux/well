part of 'generated.dart';

class GetProfileVariablesBuilder {
  String id;

  final FirebaseDataConnect _dataConnect;
  GetProfileVariablesBuilder(this._dataConnect, {required  this.id,});
  Deserializer<GetProfileData> dataDeserializer = (dynamic json)  => GetProfileData.fromJson(jsonDecode(json));
  Serializer<GetProfileVariables> varsSerializer = (GetProfileVariables vars) => jsonEncode(vars.toJson());
  Future<QueryResult<GetProfileData, GetProfileVariables>> execute({QueryFetchPolicy fetchPolicy = QueryFetchPolicy.preferCache}) {
    return ref().execute(fetchPolicy: fetchPolicy);
  }

  QueryRef<GetProfileData, GetProfileVariables> ref() {
    GetProfileVariables vars= GetProfileVariables(id: id,);
    return _dataConnect.query("GetProfile", dataDeserializer, varsSerializer, vars);
  }
}

@immutable
class GetProfileProfile {
  final String name;
  final String? metadata;
  GetProfileProfile.fromJson(dynamic json):
  
  name = nativeFromJson<String>(json['name']),
  metadata = json['metadata'] == null ? null : nativeFromJson<String>(json['metadata']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final GetProfileProfile otherTyped = other as GetProfileProfile;
    return name == otherTyped.name && 
    metadata == otherTyped.metadata;
    
  }
  @override
  int get hashCode => Object.hashAll([name.hashCode, metadata.hashCode]);
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['name'] = nativeToJson<String>(name);
    if (metadata != null) {
      json['metadata'] = nativeToJson<String?>(metadata);
    }
    return json;
  }

  GetProfileProfile({
    required this.name,
    this.metadata,
  });
}

@immutable
class GetProfileData {
  final GetProfileProfile? profile;
  GetProfileData.fromJson(dynamic json):
  
  profile = json['profile'] == null ? null : GetProfileProfile.fromJson(json['profile']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final GetProfileData otherTyped = other as GetProfileData;
    return profile == otherTyped.profile;
    
  }
  @override
  int get hashCode => profile.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    if (profile != null) {
      json['profile'] = profile!.toJson();
    }
    return json;
  }

  GetProfileData({
    this.profile,
  });
}

@immutable
class GetProfileVariables {
  final String id;
  @Deprecated('fromJson is deprecated for Variable classes as they are no longer required for deserialization.')
  GetProfileVariables.fromJson(Map<String, dynamic> json):
  
  id = nativeFromJson<String>(json['id']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final GetProfileVariables otherTyped = other as GetProfileVariables;
    return id == otherTyped.id;
    
  }
  @override
  int get hashCode => id.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    return json;
  }

  GetProfileVariables({
    required this.id,
  });
}

